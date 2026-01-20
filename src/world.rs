use crate::{AppState, AutoDespawn, CollisionLayer};
use avian3d::{
    parry::{
        math::Point,
        na::Vector3,
        shape::{SharedShape, Voxels},
    },
    prelude::{Collider, CollisionLayers, Friction, Restitution, RigidBody},
};
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    platform::collections::HashMap,
    prelude::*,
};
use serde::{Deserialize, Serialize};
use std::ops::RangeInclusive;

const CHUNK_WIDTH: usize = 16;
const CHUNK_HEIGHT: usize = 16;
const CHUNK_DEPTH: usize = 16;

// First goal is for player 0, second for player 1
pub const GOAL_BOUNDS: [(
    RangeInclusive<i32>,
    RangeInclusive<i32>,
    RangeInclusive<i32>,
); 2] = [(-27..=-25, -3..=-1, -1..=1), (25..=27, -3..=-1, -1..=1)];

#[derive(Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    pub fn get_uvs(&self) -> [(f32, f32, f32, f32); 6] {
        match self {
            BlockType::Air => [(0.0, 0.0, 0.0, 0.0); 6],
            BlockType::Grass => {
                let mut uvs = [(0.0, 0.25, 0.25, 0.0); 6];
                uvs[2] = (0.25, 0.5, 0.25, 0.0); // top
                uvs
            }
            BlockType::Dirt => [(0.5, 0.75, 0.25, 0.0); 6],
            BlockType::Stone => [(0.75, 1.0, 0.25, 0.0); 6],
            BlockType::RedBlock => [(0.25, 0.0, 0.5, 0.25); 6],
            BlockType::BlueBlock => [(0.25, 0.5, 0.5, 0.25); 6],
            BlockType::WhiteBlock => [(0.5, 0.75, 0.5, 0.25); 6],
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
        Err(())
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

                    let block_uvs = block_type.get_uvs();

                    let xf = x as f32 - 0.5;
                    let yf = y as f32 - 0.5;
                    let zf = z as f32 - 0.5;

                    // +X face (right); right = +Z, up = +Y
                    if !is_opaque(x as isize + 1, y as isize, z as isize) {
                        let (u0, u1, v0, v1) = block_uvs[0];
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
                        let (u0, u1, v0, v1) = block_uvs[1];
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
                        let (u0, u1, v0, v1) = block_uvs[2];
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
                        let (u0, u1, v0, v1) = block_uvs[3];
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
                        let (u0, u1, v0, v1) = block_uvs[4];
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
                        let (u0, u1, v0, v1) = block_uvs[5];
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
    collider: Option<Entity>,
}

impl ChunkMap {
    pub fn insert(&mut self, pos: IVec3, mut chunk: Chunk) {
        chunk.dirty = true;
        self.chunks.insert(pos, chunk);
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
            Ok(())
        } else {
            Err(())
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

pub struct WorldPlugin {
    headless: bool,
}

impl WorldPlugin {
    pub fn new(headless: bool) -> Self {
        WorldPlugin { headless }
    }
}

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        if !self.headless {
            app.add_systems(Startup, setup);
        }
        app.add_systems(Update, regen_dirty_chunks);
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
    data: Option<Res<WorldPluginData>>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut chunk_map: Single<&mut ChunkMap, Changed<ChunkMap>>,
) {
    let mut dirty = false;
    for (pos, chunk) in chunk_map.chunks.iter_mut() {
        if chunk.dirty {
            dirty = true;
            if data.is_none() || meshes.is_none() {
                chunk.dirty = false;
                continue;
            }
            if let Some(mesh_entity) = chunk.mesh {
                commands.entity(mesh_entity).despawn();
            }
            let mesh = chunk.generate_mesh();
            if mesh.count_vertices() == 0 {
                chunk.mesh = None;
                continue;
            } else {
                let mesh_entity = commands
                    .spawn((
                        Mesh3d(meshes.as_mut().unwrap().add(mesh)),
                        MeshMaterial3d(data.as_ref().unwrap().atlas_material.clone()),
                        Transform::default()
                            .with_translation(pos.as_vec3() * 16.0 + Vec3::splat(0.5)),
                        AutoDespawn(AppState::Game),
                    ))
                    .id();
                chunk.mesh = Some(mesh_entity);
            }
            // Reset dirty flag after regenerating
            chunk.dirty = false;
        }
    }

    if dirty {
        if let Some(collider_entity) = chunk_map.collider {
            commands.entity(collider_entity).despawn();
        }

        let mut points: Vec<Point<i32>> = Vec::new();
        for (pos, chunk) in chunk_map.chunks.iter() {
            for i in 0..CHUNK_WIDTH {
                for j in 0..CHUNK_HEIGHT {
                    for k in 0..CHUNK_DEPTH {
                        if chunk.blocks[i][k][j] != BlockType::Air {
                            points.push(Point::new(
                                i as i32 + pos.x * CHUNK_WIDTH as i32,
                                j as i32 + pos.y * CHUNK_HEIGHT as i32,
                                k as i32 + pos.z * CHUNK_DEPTH as i32,
                            ));
                        }
                    }
                }
            }
        }

        let block_shape = 1.0;
        let voxel_shape = Voxels::new(Vector3::new(block_shape, block_shape, block_shape), &points);
        let collider_entity = commands
            .spawn((
                RigidBody::Static,
                Collider::from(SharedShape::new(voxel_shape)),
                CollisionLayers::new(
                    CollisionLayer::World,
                    [CollisionLayer::Player, CollisionLayer::Projectile],
                ),
                Friction::new(0.0),
                Restitution::new(0.0),
                Transform::from_xyz(0.0, 0.0, 0.0),
                AutoDespawn(AppState::Game),
            ))
            .id();
        chunk_map.collider = Some(collider_entity);
    }
}

pub fn init_map() -> ChunkMap {
    let mut chunkmap = ChunkMap::default();

    for x in -2..=2 {
        for y in -1..=1 {
            for z in -1..=1 {
                chunkmap.insert((x, y, z).into(), Chunk::default());
            }
        }
    }

    for x in -20..=20 {
        for y in -8..=0 {
            chunkmap
                .set_block(
                    (x, y, 0).into(),
                    match x {
                        -20..0 => BlockType::BlueBlock,
                        0 => BlockType::WhiteBlock,
                        1..=20 => BlockType::RedBlock,
                        _ => unreachable!(),
                    },
                )
                .unwrap();
        }
    }

    for i in 0..2 {
        for x in 21..=30 {
            for y in -5..=0 {
                'outer: for z in -5..=5 {
                    for (x_range, y_range, z_range) in GOAL_BOUNDS.iter() {
                        if x_range.contains(&x)
                            && (*y_range.start()..=y_range.end() + 1).contains(&y)
                            && z_range.contains(&z)
                        {
                            continue 'outer;
                        }
                    }
                    chunkmap
                        .set_block(
                            (x * (i * 2 - 1), y, z).into(),
                            match y {
                                -5..=-3 => BlockType::Stone,
                                -2..=-1 => BlockType::Dirt,
                                0 => BlockType::Grass,
                                _ => unreachable!(),
                            },
                        )
                        .unwrap();
                }
            }
        }
    }

    chunkmap
}
