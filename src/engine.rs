use async_graphql::FieldResult;
use cgmath::Matrix4;
use lazy_static::lazy_static;
use std::{collections::HashMap, pin::Pin};
use tracing::instrument;

use crate::release::ReleaseConfig;

pub mod belt_engine;
pub mod slic3r;

/// A Slicer or other engine that converts various file formats into GCode
pub struct Engine {
    pub id: async_graphql::ID,
    pub name: &'static str,
    /// Engine-specific transforms to be applied before any model-specific transforms when
    /// exporting a mesh for slicing.
    pub transform_mat4: Matrix4<f32>,
    /// True if the engine allows parts to be positioned on the bed of the machine
    pub allows_positioning: bool,
    /// True for each axis about which the rotation direction should be visually reversed
    pub invert_rotation: InvertRotation,
    /// The file formats accepted by the engine
    pub accepted_file_formats: Vec<&'static str>,
    /// The Github releases page (if applicable)
    pub release_url: Option<&'static str>,
    pub home_page: &'static str,
    pub release_config: Pin<Box<dyn ReleaseConfig + Sync + Send + Unpin>>,
    // pub generate_gcode: &'static (dyn Fn(PathBuf, PathBuf, PathBuf) -> Pin<Box<dyn Stream<Item = Result<f32>>>>
    //               + Sync),
}

/// True for each axis about which the rotation direction should be visually reversed
#[derive(Default, async_graphql::SimpleObject)]
pub struct InvertRotation {
    x: bool,
    y: bool,
    z: bool,
}

lazy_static! {
    pub static ref ENGINES: HashMap<async_graphql::ID, Engine> = {
        let mut map = HashMap::new();

        let mut engines = vec![belt_engine::engine()];
        engines.append(&mut slic3r::engines());

        for engine in engines {
            map.insert(engine.id.clone(), engine);
        }

        map
    };
}

#[derive(Default)]
pub struct EnginesQuery;

#[async_graphql::Object]
impl EnginesQuery {
    #[instrument(skip(self))]
    async fn engines(&self) -> FieldResult<Vec<&'static Engine>> {
        Ok(ENGINES.values().collect())
    }
}

#[async_graphql::Object]
impl Engine {
    async fn id(&self) -> &async_graphql::ID {
        &self.id
    }
    async fn name(&self) -> &'static str {
        self.name
    }
    async fn allows_positioning(&self) -> bool {
        self.allows_positioning
    }
    async fn invert_rotation(&self) -> &InvertRotation {
        &self.invert_rotation
    }
    async fn accepted_file_formats(&self) -> &Vec<&'static str> {
        &self.accepted_file_formats
    }
    async fn home_page(&self) -> &str {
        &self.home_page
    }

    async fn transform_mat4(&self) -> Vec<Vec<f32>> {
        let mat4: &[[f32; 4]; 4] = self.transform_mat4.as_ref();
        mat4.into_iter()
            .map(|vec| vec.into_iter().map(|v| *v).collect())
            .collect()
    }
}
