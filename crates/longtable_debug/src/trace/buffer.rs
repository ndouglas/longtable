//! Ring buffer for trace records.
//!
//! Provides a fixed-size buffer that stores the most recent trace records,
//! with efficient tick-based indexing for queries.

use std::collections::VecDeque;

use super::record::{TraceEvent, TraceRecord};

// =============================================================================
// Trace Buffer
// =============================================================================

/// A ring buffer for storing trace records.
///
/// Maintains a fixed maximum size, discarding oldest records when full.
/// Provides efficient lookup by tick number.
#[derive(Clone, Debug)]
pub struct TraceBuffer {
    /// The records, oldest first.
    records: VecDeque<TraceRecord>,
    /// Maximum number of records to store.
    max_size: usize,
    /// Next record ID to assign.
    next_id: u64,
    /// Tick index: maps tick number to first record index for that tick.
    tick_index: Vec<(u64, usize)>,
}

impl TraceBuffer {
    /// Creates a new trace buffer with the given maximum size.
    #[must_use]
    pub fn new(max_size: usize) -> Self {
        Self {
            records: VecDeque::with_capacity(max_size.min(1024)),
            max_size,
            next_id: 0,
            tick_index: Vec::new(),
        }
    }

    /// Creates a buffer with default size (10000 records).
    #[must_use]
    pub fn default_size() -> Self {
        Self::new(10000)
    }

    /// Pushes a new event to the buffer.
    ///
    /// Returns the assigned record ID.
    pub fn push(&mut self, tick: u64, timestamp_ns: u64, event: TraceEvent) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        // Check if this is the first record for this tick
        if self.tick_index.is_empty() || self.tick_index.last().map(|(t, _)| *t) != Some(tick) {
            self.tick_index.push((tick, self.records.len()));
        }

        let record = TraceRecord::new(id, tick, timestamp_ns, event);
        self.records.push_back(record);

        // Evict oldest if over capacity
        while self.records.len() > self.max_size {
            self.records.pop_front();
            // Update tick index - remove entries that point to evicted records
            self.tick_index.retain(|(_, idx)| *idx < self.records.len());
            // Adjust remaining indices
            for (_, idx) in &mut self.tick_index {
                if *idx > 0 {
                    *idx -= 1;
                }
            }
        }

        id
    }

    /// Returns the number of records in the buffer.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns true if the buffer is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Clears all records from the buffer.
    pub fn clear(&mut self) {
        self.records.clear();
        self.tick_index.clear();
        // Don't reset next_id - keep it monotonically increasing
    }

    /// Returns an iterator over all records.
    pub fn iter(&self) -> impl Iterator<Item = &TraceRecord> {
        self.records.iter()
    }

    /// Returns records for a specific tick.
    #[must_use]
    pub fn records_for_tick(&self, tick: u64) -> Vec<&TraceRecord> {
        self.records.iter().filter(|r| r.tick == tick).collect()
    }

    /// Returns records in a tick range (inclusive).
    #[must_use]
    pub fn records_in_range(&self, start_tick: u64, end_tick: u64) -> Vec<&TraceRecord> {
        self.records
            .iter()
            .filter(|r| r.tick >= start_tick && r.tick <= end_tick)
            .collect()
    }

    /// Returns the most recent N records.
    #[must_use]
    pub fn recent(&self, count: usize) -> Vec<&TraceRecord> {
        let start = self.records.len().saturating_sub(count);
        self.records.iter().skip(start).collect()
    }

    /// Returns records matching a predicate.
    pub fn filter<F>(&self, predicate: F) -> Vec<&TraceRecord>
    where
        F: Fn(&TraceRecord) -> bool,
    {
        self.records.iter().filter(|r| predicate(r)).collect()
    }

    /// Returns records of a specific event type.
    #[must_use]
    pub fn by_event_type(&self, event_type: &str) -> Vec<&TraceRecord> {
        self.filter(|r| r.event_type() == event_type)
    }

    /// Returns the oldest tick number in the buffer.
    #[must_use]
    pub fn oldest_tick(&self) -> Option<u64> {
        self.records.front().map(|r| r.tick)
    }

    /// Returns the newest tick number in the buffer.
    #[must_use]
    pub fn newest_tick(&self) -> Option<u64> {
        self.records.back().map(|r| r.tick)
    }

    /// Returns all unique tick numbers in the buffer.
    #[must_use]
    pub fn ticks(&self) -> Vec<u64> {
        self.tick_index.iter().map(|(t, _)| *t).collect()
    }

    /// Returns statistics about the buffer.
    #[must_use]
    pub fn stats(&self) -> TraceBufferStats {
        let mut event_counts = std::collections::HashMap::new();
        for record in &self.records {
            *event_counts.entry(record.event_type()).or_insert(0) += 1;
        }

        TraceBufferStats {
            record_count: self.records.len(),
            max_size: self.max_size,
            oldest_tick: self.oldest_tick(),
            newest_tick: self.newest_tick(),
            tick_count: self.tick_index.len(),
            event_counts,
        }
    }
}

impl Default for TraceBuffer {
    fn default() -> Self {
        Self::default_size()
    }
}

// =============================================================================
// Buffer Statistics
// =============================================================================

/// Statistics about a trace buffer.
#[derive(Clone, Debug)]
pub struct TraceBufferStats {
    /// Number of records currently in buffer.
    pub record_count: usize,
    /// Maximum buffer size.
    pub max_size: usize,
    /// Oldest tick in buffer.
    pub oldest_tick: Option<u64>,
    /// Newest tick in buffer.
    pub newest_tick: Option<u64>,
    /// Number of distinct ticks.
    pub tick_count: usize,
    /// Count of each event type.
    pub event_counts: std::collections::HashMap<&'static str, usize>,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_push_and_len() {
        let mut buffer = TraceBuffer::new(100);
        assert!(buffer.is_empty());

        buffer.push(1, 1000, TraceEvent::TickStart { tick: 1 });
        assert_eq!(buffer.len(), 1);

        buffer.push(
            1,
            2000,
            TraceEvent::TickEnd {
                tick: 1,
                success: true,
            },
        );
        assert_eq!(buffer.len(), 2);
    }

    #[test]
    fn buffer_eviction() {
        let mut buffer = TraceBuffer::new(3);

        buffer.push(1, 1000, TraceEvent::TickStart { tick: 1 });
        buffer.push(
            1,
            2000,
            TraceEvent::TickEnd {
                tick: 1,
                success: true,
            },
        );
        buffer.push(2, 3000, TraceEvent::TickStart { tick: 2 });
        assert_eq!(buffer.len(), 3);

        // This should evict the oldest
        buffer.push(
            2,
            4000,
            TraceEvent::TickEnd {
                tick: 2,
                success: true,
            },
        );
        assert_eq!(buffer.len(), 3);

        // Oldest should now be tick-end for tick 1
        let oldest = buffer.records.front().unwrap();
        assert!(matches!(oldest.event, TraceEvent::TickEnd { tick: 1, .. }));
    }

    #[test]
    fn records_for_tick() {
        let mut buffer = TraceBuffer::new(100);

        buffer.push(1, 1000, TraceEvent::TickStart { tick: 1 });
        buffer.push(
            1,
            2000,
            TraceEvent::TickEnd {
                tick: 1,
                success: true,
            },
        );
        buffer.push(2, 3000, TraceEvent::TickStart { tick: 2 });

        let tick1_records = buffer.records_for_tick(1);
        assert_eq!(tick1_records.len(), 2);

        let tick2_records = buffer.records_for_tick(2);
        assert_eq!(tick2_records.len(), 1);

        let tick3_records = buffer.records_for_tick(3);
        assert!(tick3_records.is_empty());
    }

    #[test]
    fn records_in_range() {
        let mut buffer = TraceBuffer::new(100);

        for tick in 1..=5 {
            buffer.push(tick, tick * 1000, TraceEvent::TickStart { tick });
        }

        let range = buffer.records_in_range(2, 4);
        assert_eq!(range.len(), 3);
    }

    #[test]
    fn recent_records() {
        let mut buffer = TraceBuffer::new(100);

        for tick in 1..=10 {
            buffer.push(tick, tick * 1000, TraceEvent::TickStart { tick });
        }

        let recent = buffer.recent(3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].tick, 8);
        assert_eq!(recent[2].tick, 10);
    }

    #[test]
    fn by_event_type() {
        let mut buffer = TraceBuffer::new(100);

        buffer.push(1, 1000, TraceEvent::TickStart { tick: 1 });
        buffer.push(
            1,
            2000,
            TraceEvent::TickEnd {
                tick: 1,
                success: true,
            },
        );
        buffer.push(2, 3000, TraceEvent::TickStart { tick: 2 });

        let starts = buffer.by_event_type("tick-start");
        assert_eq!(starts.len(), 2);

        let ends = buffer.by_event_type("tick-end");
        assert_eq!(ends.len(), 1);
    }

    #[test]
    fn buffer_clear() {
        let mut buffer = TraceBuffer::new(100);

        buffer.push(1, 1000, TraceEvent::TickStart { tick: 1 });
        buffer.push(
            1,
            2000,
            TraceEvent::TickEnd {
                tick: 1,
                success: true,
            },
        );

        buffer.clear();
        assert!(buffer.is_empty());
        assert!(buffer.oldest_tick().is_none());
    }

    #[test]
    fn buffer_stats() {
        let mut buffer = TraceBuffer::new(100);

        buffer.push(1, 1000, TraceEvent::TickStart { tick: 1 });
        buffer.push(
            1,
            2000,
            TraceEvent::TickEnd {
                tick: 1,
                success: true,
            },
        );
        buffer.push(2, 3000, TraceEvent::TickStart { tick: 2 });

        let stats = buffer.stats();
        assert_eq!(stats.record_count, 3);
        assert_eq!(stats.oldest_tick, Some(1));
        assert_eq!(stats.newest_tick, Some(2));
        assert_eq!(stats.tick_count, 2);
        assert_eq!(stats.event_counts.get("tick-start"), Some(&2));
        assert_eq!(stats.event_counts.get("tick-end"), Some(&1));
    }
}
