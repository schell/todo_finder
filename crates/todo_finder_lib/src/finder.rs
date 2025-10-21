//! todo_finder is our broadphase TODO detector.
pub mod parse;
mod rg;
pub use rg::PossibleTodosInFile;

use crate::Error;

pub struct FileSearcher;

impl FileSearcher {
    /// Find the locations of possible TODOs at the given path.
    pub async fn find(path: &str, excludes: &[String]) -> Result<Vec<PossibleTodosInFile>, Error> {
        let output = rg::get_rg_output_with_common_patterns(path, excludes).await?;
        rg::parse_rg_output(&output)
    }
}
