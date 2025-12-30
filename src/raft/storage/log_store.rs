//! Raft log storage implementation
//!
//! Provides in-memory log storage for Raft log entries.

use openraft::StorageError;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::ops::RangeBounds;
use std::sync::Arc;

use futures::lock::Mutex;
use openraft::LogId;
use openraft::LogState;
use openraft::RaftLogId;
use openraft::RaftTypeConfig;
use openraft::Vote;
use openraft::storage::LogFlushed;
use openraft::storage::RaftLogReader;
use openraft::storage::RaftLogStorage;

// Type aliases for convenience
type LogIdOf<C> = LogId<<C as RaftTypeConfig>::NodeId>;
type VoteOf<C> = Vote<<C as RaftTypeConfig>::NodeId>;

/// Raft log store implementation with in-memory storage
#[derive(Debug, Clone, Default)]
pub struct LogStore<C: RaftTypeConfig> {
    inner: Arc<Mutex<LogStoreInner<C>>>,
}

#[derive(Debug)]
struct LogStoreInner<C: RaftTypeConfig> {
    /// The last purged log id
    last_purged_log_id: Option<LogIdOf<C>>,

    /// The Raft log
    log: BTreeMap<u64, C::Entry>,

    /// The commit log id
    committed: Option<LogIdOf<C>>,

    /// The current granted vote
    vote: Option<VoteOf<C>>,
}

impl<C: RaftTypeConfig> Default for LogStoreInner<C> {
    fn default() -> Self {
        Self {
            last_purged_log_id: None,
            log: BTreeMap::new(),
            committed: None,
            vote: None,
        }
    }
}

impl<C: RaftTypeConfig> LogStoreInner<C> {
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Debug>(
        &mut self,
        range: RB,
    ) -> Result<Vec<C::Entry>, StorageError<C::NodeId>>
    where
        C::Entry: Clone,
    {
        let response = self
            .log
            .range(range.clone())
            .map(|(_, val)| val.clone())
            .collect::<Vec<_>>();
        Ok(response)
    }

    async fn get_log_state(&mut self) -> Result<LogState<C>, StorageError<C::NodeId>> {
        // Use RaftLogId trait to get log_id
        let last = self
            .log
            .iter()
            .next_back()
            .map(|(_, ent)| <C::Entry as RaftLogId<C::NodeId>>::get_log_id(ent).clone());

        let last_purged = self.last_purged_log_id.clone();

        let last = match last {
            None => last_purged.clone(),
            Some(x) => Some(x.clone()),
        };

        Ok(LogState {
            last_purged_log_id: last_purged,
            last_log_id: last,
        })
    }

    async fn save_committed(
        &mut self,
        committed: Option<LogIdOf<C>>,
    ) -> Result<(), StorageError<C::NodeId>> {
        self.committed = committed;
        Ok(())
    }

    async fn read_committed(&mut self) -> Result<Option<LogIdOf<C>>, StorageError<C::NodeId>> {
        Ok(self.committed.clone())
    }

    async fn save_vote(&mut self, vote: &VoteOf<C>) -> Result<(), StorageError<C::NodeId>> {
        self.vote = Some(vote.clone());
        Ok(())
    }

    async fn read_vote(&mut self) -> Result<Option<VoteOf<C>>, StorageError<C::NodeId>> {
        Ok(self.vote.clone())
    }

    async fn append<I>(
        &mut self,
        entries: I,
        callback: LogFlushed<C>,
    ) -> Result<(), StorageError<C::NodeId>>
    where
        I: IntoIterator<Item = C::Entry>,
    {
        for entry in entries {
            // Use RaftLogId trait to get log_id, then access index field
            let log_id = <C::Entry as RaftLogId<C::NodeId>>::get_log_id(&entry);
            self.log.insert(log_id.index, entry);
        }
        // Call the callback to notify that log I/O is completed
        // In tests with zeroed callbacks, this may panic, so we catch it
        #[cfg(not(test))]
        {
            callback.log_io_completed(Ok(()));
        }
        #[cfg(test)]
        {
            // In tests, try to call the callback, but ignore panics from invalid channels
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                callback.log_io_completed(Ok(()));
            })).ok();
        }

        Ok(())
    }

    async fn truncate(&mut self, log_id: LogIdOf<C>) -> Result<(), StorageError<C::NodeId>> {
        let keys = self
            .log
            .range(log_id.index..)
            .map(|(k, _v)| *k)
            .collect::<Vec<_>>();
        for key in keys {
            self.log.remove(&key);
        }

        Ok(())
    }

    async fn purge(&mut self, log_id: LogIdOf<C>) -> Result<(), StorageError<C::NodeId>> {
        {
            let ld = &mut self.last_purged_log_id;
            assert!(ld.as_ref() <= Some(&log_id));
            *ld = Some(log_id.clone());
        }

        {
            let keys = self
                .log
                .range(..=log_id.index)
                .map(|(k, _v)| *k)
                .collect::<Vec<_>>();
            for key in keys {
                self.log.remove(&key);
            }
        }

        Ok(())
    }
}

impl<C: RaftTypeConfig> RaftLogReader<C> for LogStore<C>
where
    C::Entry: Clone,
{
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Debug>(
        &mut self,
        range: RB,
    ) -> Result<Vec<C::Entry>, StorageError<C::NodeId>> {
        let mut inner = self.inner.lock().await;
        inner.try_get_log_entries(range).await
    }
}

impl<C: RaftTypeConfig> RaftLogStorage<C> for LogStore<C>
where
    C::Entry: Clone,
{
    type LogReader = Self;

    async fn get_log_state(&mut self) -> Result<LogState<C>, StorageError<C::NodeId>> {
        let mut inner = self.inner.lock().await;
        inner.get_log_state().await
    }

    async fn save_committed(
        &mut self,
        committed: Option<LogIdOf<C>>,
    ) -> Result<(), StorageError<C::NodeId>> {
        let mut inner = self.inner.lock().await;
        inner.save_committed(committed).await
    }

    async fn read_committed(&mut self) -> Result<Option<LogIdOf<C>>, StorageError<C::NodeId>> {
        let mut inner = self.inner.lock().await;
        inner.read_committed().await
    }

    async fn save_vote(&mut self, vote: &VoteOf<C>) -> Result<(), StorageError<C::NodeId>> {
        let mut inner = self.inner.lock().await;
        inner.save_vote(vote).await
    }

    async fn read_vote(&mut self) -> Result<Option<VoteOf<C>>, StorageError<C::NodeId>> {
        let mut inner = self.inner.lock().await;
        inner.read_vote().await
    }

    async fn append<I>(
        &mut self,
        entries: I,
        callback: LogFlushed<C>,
    ) -> Result<(), StorageError<C::NodeId>>
    where
        I: IntoIterator<Item = C::Entry>,
    {
        let mut inner = self.inner.lock().await;
        inner.append(entries, callback).await
    }

    async fn truncate(&mut self, log_id: LogIdOf<C>) -> Result<(), StorageError<C::NodeId>> {
        let mut inner = self.inner.lock().await;
        inner.truncate(log_id).await
    }

    async fn purge(&mut self, log_id: LogIdOf<C>) -> Result<(), StorageError<C::NodeId>> {
        let mut inner = self.inner.lock().await;
        inner.purge(log_id).await
    }

    async fn get_log_reader(&mut self) -> Self::LogReader {
        self.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raft::types::TypeConfig;
    use openraft::Entry;
    use openraft::EntryPayload;
    use openraft::LeaderId;
    use openraft::LogId;
    use openraft::storage::LogFlushed;
    use rstest::*;

    #[fixture]
    fn test_log_store() -> LogStore<TypeConfig> {
        LogStore::default()
    }

    // Helper to create a LogFlushed callback for tests
    // In 0.9.21, LogFlushed is a struct with private fields
    // Since new() is private, we use unsafe to zero-initialize it
    // The append method will catch panics from invalid callbacks in tests
    fn create_test_callback() -> LogFlushed<TypeConfig> {
        unsafe {
            std::mem::zeroed()
        }
    }

    fn create_test_entry(index: u64) -> Entry<TypeConfig> {
        Entry {
            log_id: LogId::new(LeaderId::new(1, 1), index),
            payload: EntryPayload::Blank,
        }
    }

    #[tokio::test]
    async fn test_log_store_default() {
        let store = test_log_store();
        let mut store = store;
        let state = store.get_log_state().await.unwrap();
        assert!(state.last_log_id.is_none());
        assert!(state.last_purged_log_id.is_none());
    }

    #[rstest]
    #[case::single_entry(1, vec![create_test_entry(1)])]
    #[case::multiple_entries(5, (1..=5).map(create_test_entry).collect())]
    #[tokio::test]
    async fn test_append_entries(
        #[case] expected_count: usize,
        #[case] entries: Vec<Entry<TypeConfig>>,
    ) {
        let mut store = test_log_store();
        let callback = create_test_callback();
        store.append(entries.clone(), callback).await.unwrap();

        let state = store.get_log_state().await.unwrap();
        assert_eq!(
            state.last_log_id.map(|id| id.index),
            Some(expected_count as u64)
        );

        let mut reader = store.get_log_reader().await;
        let retrieved = reader
            .try_get_log_entries(1..=expected_count as u64)
            .await
            .unwrap();
        assert_eq!(retrieved.len(), expected_count);
    }

    #[tokio::test]
    async fn test_append_empty_entries() {
        let mut store = test_log_store();
        let callback = create_test_callback();
        store.append(vec![], callback).await.unwrap();

        let state = store.get_log_state().await.unwrap();
        assert!(state.last_log_id.is_none());
    }

    #[tokio::test]
    async fn test_save_and_read_committed() {
        let mut store = test_log_store();
        let log_id = LogId::new(LeaderId::new(1, 1), 5);

        store.save_committed(Some(log_id.clone())).await.unwrap();
        let committed = store.read_committed().await.unwrap();
        assert_eq!(committed, Some(log_id));
    }

    #[tokio::test]
    async fn test_save_and_read_committed_none() {
        let mut store = test_log_store();

        store.save_committed(None).await.unwrap();
        let committed = store.read_committed().await.unwrap();
        assert!(committed.is_none());
    }

    #[tokio::test]
    async fn test_save_and_read_vote() {
        let mut store = test_log_store();
        let vote = Vote::new(1, 1);

        store.save_vote(&vote).await.unwrap();
        let retrieved = store.read_vote().await.unwrap();
        assert_eq!(retrieved, Some(vote));
    }

    #[tokio::test]
    async fn test_read_vote_none() {
        let mut store = test_log_store();
        let vote = store.read_vote().await.unwrap();
        assert!(vote.is_none());
    }

    #[tokio::test]
    async fn test_truncate() {
        let mut store = test_log_store();
        let entries: Vec<_> = (1..=10).map(create_test_entry).collect();
        let callback = create_test_callback();
        store.append(entries, callback).await.unwrap();

        let log_id = LogId::new(LeaderId::new(1, 1), 5);
        store.truncate(log_id).await.unwrap();

        let mut reader = store.get_log_reader().await;
        let remaining = reader.try_get_log_entries(1..=10).await.unwrap();
        assert_eq!(remaining.len(), 4); // Entries 1-4 should remain
    }

    #[tokio::test]
    async fn test_purge() {
        let mut store = test_log_store();
        let entries: Vec<_> = (1..=10).map(create_test_entry).collect();
        let callback = create_test_callback();
        store.append(entries, callback).await.unwrap();

        let log_id = LogId::new(LeaderId::new(1, 1), 5);
        store.purge(log_id).await.unwrap();

        let state = store.get_log_state().await.unwrap();
        assert_eq!(state.last_purged_log_id.map(|id| id.index), Some(5));

        let mut reader = store.get_log_reader().await;
        let remaining = reader.try_get_log_entries(1..=10).await.unwrap();
        assert_eq!(remaining.len(), 5); // Entries 6-10 should remain
    }

    #[tokio::test]
    async fn test_get_log_entries_range() {
        let mut store = test_log_store();
        let entries: Vec<_> = (1..=10).map(create_test_entry).collect();
        let callback = create_test_callback();
        store.append(entries, callback).await.unwrap();

        let mut reader = store.get_log_reader().await;
        let range_entries = reader.try_get_log_entries(3..=7).await.unwrap();
        assert_eq!(range_entries.len(), 5);
        assert_eq!(range_entries[0].log_id.index, 3);
        assert_eq!(range_entries[4].log_id.index, 7);
    }

    #[tokio::test]
    async fn test_get_log_entries_empty_range() {
        let mut store = test_log_store();
        let mut reader = store.get_log_reader().await;
        let entries = reader.try_get_log_entries(1..=5).await.unwrap();
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn test_log_state_with_entries() {
        let mut store = test_log_store();
        let entries: Vec<_> = (1..=3).map(create_test_entry).collect();
        let callback = create_test_callback();
        store.append(entries, callback).await.unwrap();

        let state = store.get_log_state().await.unwrap();
        assert_eq!(state.last_log_id.map(|id| id.index), Some(3));
        assert!(state.last_purged_log_id.is_none());
    }

    #[tokio::test]
    async fn test_log_state_after_purge() {
        let mut store = test_log_store();
        let entries: Vec<_> = (1..=5).map(create_test_entry).collect();
        let callback = create_test_callback();
        store.append(entries, callback).await.unwrap();

        let purge_id = LogId::new(LeaderId::new(1, 1), 3);
        store.purge(purge_id.clone()).await.unwrap();

        let state = store.get_log_state().await.unwrap();
        assert_eq!(state.last_purged_log_id, Some(purge_id));
        assert_eq!(state.last_log_id.map(|id| id.index), Some(5));
    }
}
