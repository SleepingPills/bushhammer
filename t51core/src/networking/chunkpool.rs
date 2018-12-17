use crate::networking::chunk::Chunk;

/// Simple pool of chunks.
pub struct ChunkPool {
    pool: Vec<Chunk>,
}

impl ChunkPool {
    pub fn new() -> ChunkPool {
        ChunkPool { pool: Vec::new() }
    }

    /// Creates a new chunk if there are none available. Provides an existing one otherwise.
    pub fn alloc(&mut self) -> Chunk {
        self.pool.pop().unwrap_or_else(|| Chunk::new())
    }

    /// Reclaim the supplied chunk into the pool.
    pub fn reclaim(&mut self, chunk: Chunk) {
        self.pool.push(chunk)
    }
}
