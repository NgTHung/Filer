pub mod cache;
pub mod navigator;
pub mod previewer;
pub mod scanner;
pub mod searcher;
pub mod watcher;

/// Trait for all actors
pub trait Actor: Send + 'static {
    /// Start the actor's main loop
    fn run(self) -> impl std::future::Future<Output = ()> + Send;
    
    /// Get actor name for logging
    fn name(&self) -> &'static str;
}