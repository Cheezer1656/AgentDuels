use avian3d::prelude::{Collider, RigidBody};
use bevy::{asset::RenderAssetUsages, mesh::{Indices, PrimitiveTopology}, platform::collections::HashMap, prelude::*};

use crate::{AppState, AutoDespawn};

const CHUNK_WIDTH: usize = 16;
const CHUNK_HEIGHT: usize = 16;
const CHUNK_DEPTH: usize = 16;

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockType {
    #[default]
    Air,
    Grass,
    Dirt,
    Stone,
    RedBlock,
    BlueBlock,
    WhiteBlock,
}

impl BlockType {
    pub fn get_uv(&self) -> (f32, f32, f32, f32) {
        match self {
            BlockType::Air => (0.0, 0.0, 0.0, 0.0),
            BlockType::Grass => (0.0, 0.25, 0.25, 0.0),
            BlockType::Dirt => (0.25, 0.5, 0.25, 0.0),
            BlockType::Stone => (0.5, 0.75, 0.25, 0.0),
            BlockType::RedBlock => (0.25, 0.0, 0.5, 0.25),
            BlockType::BlueBlock => (0.25, 0.5, 0.5, 0.25),
            BlockType::WhiteBlock => (0.5, 0.75, 0.5, 0.25),
        }
    }
}

#[derive(Default)]
pub struct Chunk {
    blocks: [[[BlockType; CHUNK_HEIGHT]; CHUNK_DEPTH]; CHUNK_WIDTH],
    mesh: Option<Entity>,
    dirty: bool,
}

impl Chunk {
    pub fn set_block(
        &mut self,
        x: usize,
        y: usize,
        z: usize,
        block_type: BlockType,
    ) -> Result<(), ()> {
        if x < CHUNK_WIDTH && y < CHUNK_HEIGHT && z < CHUNK_DEPTH {
            self.blocks[x][z][y] = block_type;
            return Ok(());
        }
        return Err(());
    }
    pub fn get_block(&self, x: usize, y: usize, z: usize) -> BlockType {
        if x < CHUNK_WIDTH && y < CHUNK_HEIGHT && z < CHUNK_DEPTH {
            self.blocks[x][z][y]
        } else {
            BlockType::Air
        }
    }
    pub fn generate_mesh(&self) -> Mesh {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );

        // Typed attribute buffers
        let mut positions: Vec<[f32; 3]> = Vec::new();
        let mut normals: Vec<[f32; 3]> = Vec::new();
        let mut uvs: Vec<[f32; 2]> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        // Helper to check if a neighbor is opaque (i.e., inside bounds and not Air)
        let is_opaque = |x: isize, y: isize, z: isize| -> bool {
            if x < 0
                || y < 0
                || z < 0
                || x as usize >= CHUNK_WIDTH
                || y as usize >= CHUNK_HEIGHT
                || z as usize >= CHUNK_DEPTH
            {
                return false; // out of bounds -> treat as Air
            }
            self.blocks[x as usize][z as usize][y as usize] != BlockType::Air
        };

        // Per-face UV push (4 verts, 6 indices)
        let mut push_face =
            |face_positions: [[f32; 3]; 4], normal: [f32; 3], face_uvs: [[f32; 2]; 4]| {
                let base = positions.len() as u32;
                positions.extend_from_slice(&face_positions);
                normals.extend_from_slice(&[normal; 4]);
                uvs.extend_from_slice(&face_uvs);
                indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            };

        for x in 0..CHUNK_WIDTH {
            for y in 0..CHUNK_HEIGHT {
                for z in 0..CHUNK_DEPTH {
                    let block_type = self.blocks[x][z][y];
                    if block_type == BlockType::Air {
                        continue;
                    }

                    let (u0, u1, v0, v1) = block_type.get_uv();

                    let xf = x as f32 - 0.5;
                    let yf = y as f32 - 0.5;
                    let zf = z as f32 - 0.5;

                    // +X face (right); right = +Z, up = +Y
                    if !is_opaque(x as isize + 1, y as isize, z as isize) {
                        push_face(
                            [
                                [xf + 1.0, yf + 0.0, zf + 0.0],
                                [xf + 1.0, yf + 1.0, zf + 0.0],
                                [xf + 1.0, yf + 1.0, zf + 1.0],
                                [xf + 1.0, yf + 0.0, zf + 1.0],
                            ],
                            [1.0, 0.0, 0.0],
                            // u along +Z, v along +Y (upright)
                            [[u0, v0], [u0, v1], [u1, v1], [u1, v0]],
                        );
                    }
                    // -X face (left); right = -Z, up = +Y
                    if !is_opaque(x as isize - 1, y as isize, z as isize) {
                        push_face(
                            [
                                [xf + 0.0, yf + 0.0, zf + 1.0],
                                [xf + 0.0, yf + 1.0, zf + 1.0],
                                [xf + 0.0, yf + 1.0, zf + 0.0],
                                [xf + 0.0, yf + 0.0, zf + 0.0],
                            ],
                            [-1.0, 0.0, 0.0],
                            // u along -Z, v along +Y (upright)
                            [[u0, v0], [u0, v1], [u1, v1], [u1, v0]],
                        );
                    }
                    // +Y face (top); right = +X, up = +Z
                    if !is_opaque(x as isize, y as isize + 1, z as isize) {
                        push_face(
                            [
                                [xf + 0.0, yf + 1.0, zf + 0.0],
                                [xf + 0.0, yf + 1.0, zf + 1.0],
                                [xf + 1.0, yf + 1.0, zf + 1.0],
                                [xf + 1.0, yf + 1.0, zf + 0.0],
                            ],
                            [0.0, 1.0, 0.0],
                            // u along +Z? Choose your preferred top orientation; this keeps it consistent.
                            [[u0, v0], [u0, v1], [u1, v1], [u1, v0]],
                        );
                    }
                    // -Y face (bottom); right = +X, up = -Z
                    if !is_opaque(x as isize, y as isize - 1, z as isize) {
                        push_face(
                            [
                                [xf + 0.0, yf + 0.0, zf + 0.0],
                                [xf + 1.0, yf + 0.0, zf + 0.0],
                                [xf + 1.0, yf + 0.0, zf + 1.0],
                                [xf + 0.0, yf + 0.0, zf + 1.0],
                            ],
                            [0.0, -1.0, 0.0],
                            [[u0, v0], [u1, v0], [u1, v1], [u0, v1]],
                        );
                    }
                    // +Z face (front); right = -X, up = +Y
                    if !is_opaque(x as isize, y as isize, z as isize + 1) {
                        push_face(
                            [
                                [xf + 0.0, yf + 0.0, zf + 1.0],
                                [xf + 1.0, yf + 0.0, zf + 1.0],
                                [xf + 1.0, yf + 1.0, zf + 1.0],
                                [xf + 0.0, yf + 1.0, zf + 1.0],
                            ],
                            [0.0, 0.0, 1.0],
                            // u along -X, v along +Y (upright)
                            [[u1, v0], [u0, v0], [u0, v1], [u1, v1]],
                        );
                    }
                    // -Z face (back); right = +X, up = +Y
                    if !is_opaque(x as isize, y as isize, z as isize - 1) {
                        push_face(
                            [
                                [xf + 1.0, yf + 0.0, zf + 0.0],
                                [xf + 0.0, yf + 0.0, zf + 0.0],
                                [xf + 0.0, yf + 1.0, zf + 0.0],
                                [xf + 1.0, yf + 1.0, zf + 0.0],
                            ],
                            [0.0, 0.0, -1.0],
                            // u along +X, v along +Y (upright)
                            [[u1, v0], [u0, v0], [u0, v1], [u1, v1]],
                        );
                    }
                }
            }
        }

        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh.insert_indices(Indices::U32(indices));

        mesh
    }
}

#[derive(Component, Default)]
pub struct ChunkMap {
    chunks: HashMap<IVec3, Chunk>,
}

impl ChunkMap {
    pub fn insert(&mut self, pos: IVec3, mut chunk: Chunk) {
        chunk.dirty = true;
        self.chunks.insert(pos, chunk);
    }
    pub fn get(&self, pos: IVec3) -> Option<&Chunk> {
        self.chunks.get(&pos)
    }
    pub fn set_block(&mut self, pos: IVec3, block_type: BlockType) -> Result<(), ()> {
        let (chunk_pos, local_pos) = Self::split_pos(pos);
        let Some(chunk) = self.chunks.get_mut(&chunk_pos) else {
            return Err(());
        };
        if chunk
            .set_block(
                local_pos.x as usize,
                local_pos.y as usize,
                local_pos.z as usize,
                block_type,
            )
            .is_ok()
        {
            chunk.dirty = true;
            return Ok(());
        } else {
            return Err(());
        }
    }
    pub fn get_block(&self, pos: IVec3) -> BlockType {
        let (chunk_pos, local_pos) = Self::split_pos(pos);
        if let Some(chunk) = self.chunks.get(&chunk_pos) {
            return chunk.get_block(
                local_pos.x as usize,
                local_pos.y as usize,
                local_pos.z as usize,
            );
        }
        BlockType::Air
    }
    fn split_pos(pos: IVec3) -> (IVec3, IVec3) {
        let chunk_size = IVec3::new(CHUNK_WIDTH as i32, CHUNK_HEIGHT as i32, CHUNK_DEPTH as i32);
        let chunk_pos = pos.div_euclid(chunk_size);
        let local_pos = pos.rem_euclid(chunk_size);
        (chunk_pos, local_pos)
    }
}

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(Update, regen_dirty_chunks);
    }
}

#[derive(Resource)]
struct WorldPluginData {
    atlas_material: Handle<StandardMaterial>,
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(WorldPluginData {
        atlas_material: materials.add(StandardMaterial {
            base_color_texture: Some(asset_server.load("textures/atlas.png")),
            reflectance: 0.0,
            perceptual_roughness: 1.0,
            ..default()
        }),
    });
}

fn regen_dirty_chunks(
    mut commands: Commands,
    data: Res<WorldPluginData>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut query: Query<&mut ChunkMap, Changed<ChunkMap>>,
) {
    for mut chunk_map in query.iter_mut() {
        for (pos, chunk) in chunk_map.chunks.iter_mut() {
            if chunk.dirty {
                if let Some(mesh_entity) = chunk.mesh {
                    commands.entity(mesh_entity).despawn();
                }
                let mesh = chunk.generate_mesh();
                if mesh.count_vertices() == 0 {
                    chunk.mesh = None;
                    continue;
                } else {
                    let collider = Collider::trimesh_from_mesh(&mesh).unwrap();
                    let mesh_entity = commands
                        .spawn((
                            RigidBody::Static,
                            collider,
                            Mesh3d(meshes.add(mesh)),
                            MeshMaterial3d(data.atlas_material.clone()),
                            Transform::default().with_translation(pos.as_vec3() * 16.0),
                            AutoDespawn(AppState::Game),
                        ))
                        .id();
                    chunk.mesh = Some(mesh_entity);
                }
                // Reset dirty flag after regenerating
                chunk.dirty = false;
            }
        }
    }
}
