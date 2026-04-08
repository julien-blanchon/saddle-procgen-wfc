use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    WfcAdjacencyRule, WfcDirection, WfcRuleset, WfcTileDefinition, WfcTileId, WfcTileSymmetry,
    WfcTopology,
};

/// A unique socket type identifier.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Reflect, Serialize, Deserialize)]
pub struct WfcSocketId(pub u16);

impl From<u16> for WfcSocketId {
    fn from(value: u16) -> Self {
        Self(value)
    }
}

impl std::fmt::Display for WfcSocketId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Socket({})", self.0)
    }
}

/// Builder that collects tiles with per-face socket assignments,
/// then compiles to a [`WfcRuleset`] with automatically derived adjacency rules.
///
/// Two tiles can be adjacent in a given direction if and only if the socket on
/// the source tile's face matches the socket on the neighbor tile's opposite face.
///
/// By default, sockets are **symmetric**: socket "A" on `+X` connects to socket
/// "A" on `-X`. For asymmetric connections (e.g. pipe input → pipe output), use
/// [`add_asymmetric_pair`](WfcSocketRulesetBuilder::add_asymmetric_pair).
#[derive(Clone, Debug)]
pub struct WfcSocketRulesetBuilder {
    topology: WfcTopology,
    tiles: Vec<SocketTileEntry>,
    socket_names: BTreeMap<String, WfcSocketId>,
    /// Asymmetric pairs: (from_socket, to_socket). `from` on a face only matches
    /// `to` on the opposite face, NOT `from`.
    asymmetric_pairs: Vec<(WfcSocketId, WfcSocketId)>,
    next_socket_id: u16,
}

#[derive(Clone, Debug)]
struct SocketTileEntry {
    definition: WfcTileDefinition,
    face_sockets: BTreeMap<WfcDirection, WfcSocketId>,
}

/// Per-tile builder returned by [`WfcSocketRulesetBuilder::add_tile`].
///
/// Assign sockets to each face direction, then call [`done`](SocketTileBuilder::done)
/// to return to the parent builder.
pub struct SocketTileBuilder<'a> {
    builder: &'a mut WfcSocketRulesetBuilder,
    tile_index: usize,
}

impl WfcSocketRulesetBuilder {
    pub fn new(topology: WfcTopology) -> Self {
        Self {
            topology,
            tiles: Vec::new(),
            socket_names: BTreeMap::new(),
            asymmetric_pairs: Vec::new(),
            next_socket_id: 0,
        }
    }

    /// Register or look up a socket by name. Returns the stable socket ID.
    pub fn socket_id(&mut self, name: &str) -> WfcSocketId {
        if let Some(&id) = self.socket_names.get(name) {
            return id;
        }
        let id = WfcSocketId(self.next_socket_id);
        self.next_socket_id += 1;
        self.socket_names.insert(name.to_string(), id);
        id
    }

    /// Declare an asymmetric socket pair: a face with `from` only connects to
    /// a neighbor face with `to` (and vice versa). Neither connects to itself.
    pub fn add_asymmetric_pair(mut self, from: &str, to: &str) -> Self {
        let from_id = self.socket_id(from);
        let to_id = self.socket_id(to);
        self.asymmetric_pairs.push((from_id, to_id));
        self
    }

    /// Begin adding a tile. Returns a [`SocketTileBuilder`] for chaining socket assignments.
    pub fn add_tile(
        &mut self,
        id: impl Into<WfcTileId>,
        weight: f32,
        label: impl Into<String>,
    ) -> SocketTileBuilder<'_> {
        let definition = WfcTileDefinition::new(id, weight, label);
        self.tiles.push(SocketTileEntry {
            definition,
            face_sockets: BTreeMap::new(),
        });
        let index = self.tiles.len() - 1;
        SocketTileBuilder {
            builder: self,
            tile_index: index,
        }
    }

    /// Compile socket definitions into a standard [`WfcRuleset`].
    ///
    /// Validates that every tile has a socket for every active direction, then
    /// derives adjacency rules from socket compatibility.
    pub fn build(&self) -> Result<WfcRuleset, String> {
        let directions = WfcDirection::active(self.topology);

        if self.tiles.is_empty() {
            return Err("socket ruleset must contain at least one tile".to_string());
        }

        // Validate: every tile must have a socket for every active direction.
        for entry in &self.tiles {
            for &dir in directions {
                if !entry.face_sockets.contains_key(&dir) {
                    return Err(format!(
                        "tile {:?} ({}) is missing a socket for direction {}",
                        entry.definition.id, entry.definition.label, dir
                    ));
                }
            }
        }

        // Build the asymmetric lookup: socket A -> set of compatible sockets on
        // the opposite face. Symmetric sockets match themselves. Asymmetric
        // pairs only match their declared partner.
        let asymmetric_set: BTreeSet<WfcSocketId> = self
            .asymmetric_pairs
            .iter()
            .flat_map(|(a, b)| [*a, *b])
            .collect();

        let mut compatible: BTreeMap<WfcSocketId, BTreeSet<WfcSocketId>> = BTreeMap::new();

        // All known sockets (symmetric ones match themselves)
        let all_sockets: BTreeSet<WfcSocketId> = self
            .tiles
            .iter()
            .flat_map(|entry| entry.face_sockets.values().copied())
            .collect();

        for &socket in &all_sockets {
            if !asymmetric_set.contains(&socket) {
                compatible.entry(socket).or_default().insert(socket);
            }
        }

        // Asymmetric pairs
        for &(from, to) in &self.asymmetric_pairs {
            compatible.entry(from).or_default().insert(to);
            compatible.entry(to).or_default().insert(from);
        }

        // Derive adjacency rules. For each (source, direction), check all
        // rotation variants of each candidate tile — the solver expands
        // rotations, so a candidate that only matches after rotation is valid.
        let definitions: Vec<WfcTileDefinition> =
            self.tiles.iter().map(|e| e.definition.clone()).collect();
        let mut adjacency = Vec::new();

        for (source_idx, source) in self.tiles.iter().enumerate() {
            let source_id = source.definition.id;
            for &dir in directions {
                let source_socket = source.face_sockets[&dir];
                let compatible_sockets = compatible.get(&source_socket);

                let opposite_dir = dir.opposite();
                let allowed: Vec<WfcTileId> = self
                    .tiles
                    .iter()
                    .filter(|candidate| {
                        let unique_rotations =
                            candidate.definition.symmetry.unique_rotations(self.topology);
                        (0..unique_rotations).any(|rot| {
                            let candidate_socket =
                                rotated_socket(candidate, opposite_dir, rot, self.topology);
                            candidate_socket.is_some_and(|s| {
                                compatible_sockets.is_some_and(|set| set.contains(&s))
                            })
                        })
                    })
                    .map(|candidate| candidate.definition.id)
                    .collect();

                if allowed.is_empty() {
                    return Err(format!(
                        "tile {:?} ({}) has socket {} on {} which matches no other tile (even after considering rotated candidates)",
                        source_id, self.tiles[source_idx].definition.label, source_socket, dir
                    ));
                }

                adjacency.push(WfcAdjacencyRule::new(source_id, dir, allowed));
            }
        }

        Ok(WfcRuleset {
            topology: self.topology,
            tiles: definitions,
            adjacency,
        })
    }
}

impl<'a> SocketTileBuilder<'a> {
    /// Assign a named socket to a face direction.
    pub fn socket(self, direction: WfcDirection, name: &str) -> Self {
        let socket_id = self.builder.socket_id(name);
        self.builder.tiles[self.tile_index]
            .face_sockets
            .insert(direction, socket_id);
        self
    }

    /// Assign the same socket to all active directions for this topology.
    pub fn all_sockets(self, name: &str) -> Self {
        let socket_id = self.builder.socket_id(name);
        let directions: Vec<WfcDirection> =
            WfcDirection::active(self.builder.topology).to_vec();
        for dir in directions {
            self.builder.tiles[self.tile_index]
                .face_sockets
                .insert(dir, socket_id);
        }
        self
    }

    /// Set the tile symmetry (for auto-rotation support).
    pub fn symmetry(self, symmetry: WfcTileSymmetry) -> Self {
        self.builder.tiles[self.tile_index].definition.symmetry = symmetry;
        self
    }

    /// Finish this tile and return to the parent builder.
    pub fn done(self) -> &'a mut WfcSocketRulesetBuilder {
        self.builder
    }
}

/// Get a tile's socket on `world_direction` when placed at `rotation_steps`.
///
/// For rotation 0 or non-Cartesian2d topologies, this is just the canonical socket.
/// For rotated variants, we inverse-rotate the world direction back to the canonical
/// direction and look up that socket.
fn rotated_socket(
    entry: &SocketTileEntry,
    world_direction: WfcDirection,
    rotation_steps: u8,
    topology: WfcTopology,
) -> Option<WfcSocketId> {
    if topology != WfcTopology::Cartesian2d || rotation_steps == 0 {
        return entry.face_sockets.get(&world_direction).copied();
    }
    let canonical_dir = inverse_rotate_2d(world_direction, rotation_steps);
    entry.face_sockets.get(&canonical_dir).copied()
}

/// Inverse-rotate a Cartesian2d direction: given a world direction and rotation
/// steps, return the canonical direction that maps to it.
fn inverse_rotate_2d(direction: WfcDirection, rotation_steps: u8) -> WfcDirection {
    let mut d = direction;
    for _ in 0..((4 - rotation_steps % 4) % 4) {
        d = rotate_cw(d);
    }
    d
}

fn rotate_cw(d: WfcDirection) -> WfcDirection {
    match d {
        WfcDirection::XPos => WfcDirection::YNeg,
        WfcDirection::YNeg => WfcDirection::XNeg,
        WfcDirection::XNeg => WfcDirection::YPos,
        WfcDirection::YPos => WfcDirection::XPos,
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{WfcGridSize, WfcRequest, WfcSeed, solve_wfc};

    #[test]
    fn socket_builder_produces_valid_ruleset() {
        let mut builder = WfcSocketRulesetBuilder::new(WfcTopology::Cartesian2d);
        builder
            .add_tile(0u16, 3.0, "Grass")
            .all_sockets("grass")
            .done();
        builder
            .add_tile(1u16, 1.0, "Road")
            .socket(WfcDirection::XPos, "road")
            .socket(WfcDirection::XNeg, "road")
            .socket(WfcDirection::YPos, "grass")
            .socket(WfcDirection::YNeg, "grass")
            .done();
        let ruleset = builder.build().expect("should build");

        assert_eq!(ruleset.tiles.len(), 2);
        assert_eq!(ruleset.adjacency.len(), 8); // 2 tiles * 4 directions
    }

    #[test]
    fn socket_built_ruleset_solves() {
        let mut builder = WfcSocketRulesetBuilder::new(WfcTopology::Cartesian2d);
        builder
            .add_tile(0u16, 5.0, "Grass")
            .all_sockets("g")
            .done();
        builder
            .add_tile(1u16, 2.0, "Road")
            .socket(WfcDirection::XPos, "road")
            .socket(WfcDirection::XNeg, "road")
            .socket(WfcDirection::YPos, "g")
            .socket(WfcDirection::YNeg, "g")
            .done();
        builder
            .add_tile(2u16, 1.0, "Water")
            .socket(WfcDirection::XPos, "water")
            .socket(WfcDirection::XNeg, "water")
            .socket(WfcDirection::YPos, "g")
            .socket(WfcDirection::YNeg, "g")
            .done();

        let ruleset = builder.build().expect("should build");
        let request = WfcRequest::new(WfcGridSize::new_2d(10, 10), ruleset, WfcSeed(42));
        let solution = solve_wfc(&request).expect("should solve");
        assert_eq!(solution.grid.tiles.len(), 100);
    }

    #[test]
    fn asymmetric_sockets_enforce_directionality() {
        let mut builder =
            WfcSocketRulesetBuilder::new(WfcTopology::Cartesian2d).add_asymmetric_pair("pipe_in", "pipe_out");
        builder
            .add_tile(0u16, 3.0, "Ground")
            .all_sockets("g")
            .done();
        builder
            .add_tile(1u16, 1.0, "PipeSource")
            .socket(WfcDirection::XPos, "pipe_out")
            .socket(WfcDirection::XNeg, "g")
            .socket(WfcDirection::YPos, "g")
            .socket(WfcDirection::YNeg, "g")
            .done();
        builder
            .add_tile(2u16, 1.0, "PipeSink")
            .socket(WfcDirection::XPos, "g")
            .socket(WfcDirection::XNeg, "pipe_in")
            .socket(WfcDirection::YPos, "g")
            .socket(WfcDirection::YNeg, "g")
            .done();

        let ruleset = builder.build().expect("should build");

        // PipeSource.XPos (pipe_out) should match PipeSink.XNeg (pipe_in)
        let source_xpos = ruleset
            .adjacency
            .iter()
            .find(|r| r.tile == WfcTileId(1) && r.direction == WfcDirection::XPos)
            .expect("rule should exist");
        assert!(source_xpos.allowed_tiles.contains(&WfcTileId(2)));
        // pipe_out should NOT match another pipe_out
        assert!(!source_xpos.allowed_tiles.contains(&WfcTileId(1)));
    }

    #[test]
    fn missing_socket_produces_error() {
        let mut builder = WfcSocketRulesetBuilder::new(WfcTopology::Cartesian2d);
        builder
            .add_tile(0u16, 1.0, "Incomplete")
            .socket(WfcDirection::XPos, "a")
            .socket(WfcDirection::XNeg, "a")
            // Missing YPos, YNeg
            .done();
        let result = builder.build();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("missing a socket"));
    }

    #[test]
    fn socket_builder_with_rotation() {
        let mut builder = WfcSocketRulesetBuilder::new(WfcTopology::Cartesian2d);
        builder
            .add_tile(0u16, 3.0, "Grass")
            .all_sockets("g")
            .done();
        builder
            .add_tile(1u16, 1.0, "Straight Road")
            .socket(WfcDirection::XPos, "g")
            .socket(WfcDirection::XNeg, "g")
            .socket(WfcDirection::YPos, "road")
            .socket(WfcDirection::YNeg, "road")
            .symmetry(WfcTileSymmetry::Rotate2)
            .done();

        let ruleset = builder.build().expect("should build");
        let request = WfcRequest::new(WfcGridSize::new_2d(8, 8), ruleset, WfcSeed(7));
        let solution = solve_wfc(&request).expect("should solve");
        assert_eq!(solution.grid.tiles.len(), 64);
    }

    #[test]
    fn socket_builder_hex_topology() {
        let mut builder = WfcSocketRulesetBuilder::new(WfcTopology::Hex2d);
        builder
            .add_tile(0u16, 1.0, "Plains")
            .socket(WfcDirection::HexEast, "any")
            .socket(WfcDirection::HexWest, "any")
            .socket(WfcDirection::HexNorthEast, "any")
            .socket(WfcDirection::HexNorthWest, "any")
            .socket(WfcDirection::HexSouthEast, "any")
            .socket(WfcDirection::HexSouthWest, "any")
            .done();

        let ruleset = builder.build().expect("should build");
        let request = WfcRequest::new(WfcGridSize::new_2d(6, 6), ruleset, WfcSeed(99));
        let solution = solve_wfc(&request).expect("should solve");
        assert_eq!(solution.grid.tiles.len(), 36);
    }
}
