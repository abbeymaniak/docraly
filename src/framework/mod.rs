pub mod laravel;

use crate::discovery::Framework;
use crate::model::ProjectModel;

/// Trait implemented by framework-specific adapters (Laravel, Symfony, FastApi, etc.)
/// to extract higher-level architecture concepts like routes, middleware, and controllers.
pub trait FrameworkAdapter {
    /// Returns the framework handled by this adapter.
    #[allow(dead_code)]
    fn framework(&self) -> Framework;

    /// Analyzes the project model to discover routes and link them to controllers/actions.
    fn analyze(&self, project: &mut ProjectModel);
}
