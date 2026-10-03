use bunshin_core::{
    Tuning, UnixMillis,
    day::{
        Day, TaskKind, TaskOrigin,
        file::DayFile,
        store::{DayLock, DayStore, DayStoreError, validate_date},
    },
};
use jiff::civil::{Date, date};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, PoisonError},
};

/// Whole-day in-memory storage with a lifetime lock. Clones share files and the
/// lock, as independent handles to the same directory do. Undo is not persisted.
#[derive(Debug, Clone)]
pub struct InMemoryDayStore {
    days: Arc<Mutex<BTreeMap<Date, Result<DayFile, DayStoreError>>>>,
    locked: Arc<Mutex<bool>>,
    tuning: Tuning,
}
impl Default for InMemoryDayStore {
    fn default() -> Self {
        Self::new(Tuning::default())
    }
}
impl InMemoryDayStore {
    /// Empty directory with the supplied domain bounds.
    #[must_use]
    pub fn new(tuning: Tuning) -> Self {
        Self {
            days: Arc::default(),
            locked: Arc::default(),
            tuning,
        }
    }
    /// Seed a refused file; saves must preserve it, just as the real store does.
    pub fn seed_error(&self, date: Date, error: DayStoreError) {
        self.days
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(date, Err(error));
    }
}
struct MemoryLock(Arc<Mutex<bool>>);
impl DayLock for MemoryLock {}
impl Drop for MemoryLock {
    fn drop(&mut self) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = false;
    }
}
impl DayStore for InMemoryDayStore {
    fn load(&self, date: Date) -> Result<Day, DayStoreError> {
        let stored = self
            .days
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&date)
            .cloned();
        match stored {
            None => Ok(Day::new(date, self.tuning)),
            Some(file) => validate_date(file?.into_day(self.tuning)?, date),
        }
    }
    fn save(&self, day: &Day) -> Result<(), DayStoreError> {
        let mut days = self.days.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(file) = days.get(&day.date()) {
            validate_date(file.clone()?.into_day(self.tuning)?, day.date())?;
        }
        days.insert(day.date(), Ok(DayFile::from(day)));
        Ok(())
    }
    fn last_before(&self, date: Date) -> Result<Option<Day>, DayStoreError> {
        let last = self
            .days
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .range(..date)
            .next_back()
            .map(|(key, _)| *key);
        last.map(|date| self.load(date)).transpose()
    }
    fn take_lock(&self) -> Result<Box<dyn DayLock>, DayStoreError> {
        let mut locked = self.locked.lock().unwrap_or_else(PoisonError::into_inner);
        if *locked {
            return Err(DayStoreError::AlreadyLocked { pid: Some(1) });
        }
        *locked = true;
        Ok(Box::new(MemoryLock(Arc::clone(&self.locked))))
    }
}

/// An inaccessible directory: every operation returns `Unavailable`. It intentionally
/// fails the successful-store contract and never mutates caller data.
#[derive(Debug, Clone, Copy)]
pub struct FailingDayStore;
impl DayStore for FailingDayStore {
    fn load(&self, _: Date) -> Result<Day, DayStoreError> {
        Err(DayStoreError::Unavailable)
    }
    fn save(&self, _: &Day) -> Result<(), DayStoreError> {
        Err(DayStoreError::Unavailable)
    }
    fn last_before(&self, _: Date) -> Result<Option<Day>, DayStoreError> {
        Err(DayStoreError::Unavailable)
    }
    fn take_lock(&self) -> Result<Box<dyn DayLock>, DayStoreError> {
        Err(DayStoreError::Unavailable)
    }
}

/// Successful stores share empty reads, whole-day replacement, session-only undo,
/// strict latest-before lookup, single writer refusal and drop-based release.
/// # Panics
/// If an implementation violates the port's promises.
pub fn day_store_contract(mut make: impl FnMut() -> Box<dyn DayStore>) {
    let store = make();
    let today = date(2026, 10, 2);
    let empty = Day::new(today, Tuning::default());
    assert_eq!(store.load(today), Ok(empty.clone()));
    assert_eq!(store.last_before(today), Ok(None));
    assert_eq!(store.save(&empty), Ok(()));
    assert_eq!(store.last_before(today), Ok(None));
    let changed = match empty.add(
        "a task".into(),
        TaskKind::Untimed,
        None,
        TaskOrigin::Key,
        UnixMillis(42),
    ) {
        Ok((day, _)) => day,
        Err(error) => panic!("fixture failed: {error:?}"),
    };
    assert_eq!(store.save(&changed), Ok(()));
    match store.load(today) {
        Ok(day) => {
            assert_eq!(day.data(), changed.data());
            assert_eq!(
                day.undo(UnixMillis(43)),
                Err(bunshin_core::day::DayError::NothingToUndo)
            );
        }
        Err(error) => panic!("load failed: {error:?}"),
    }
    for before in [date(2026, 9, 28), date(2026, 10, 1)] {
        assert_eq!(store.save(&Day::new(before, Tuning::default())), Ok(()));
    }
    assert_eq!(
        store.last_before(today),
        Ok(Some(Day::new(date(2026, 10, 1), Tuning::default())))
    );
    assert_eq!(store.last_before(date(2026, 9, 28)), Ok(None));
    let first = match store.take_lock() {
        Ok(lock) => lock,
        Err(error) => panic!("lock failed: {error:?}"),
    };
    assert!(matches!(
        store.take_lock(),
        Err(DayStoreError::AlreadyLocked { .. })
    ));
    assert_eq!(
        store.load(today).map(|day| day.data().clone()),
        Ok(changed.data().clone())
    );
    drop(first);
    assert!(store.take_lock().is_ok());
}

/// A pre-seeded unreadable or unsupported file stays refused after a save attempt.
/// Both the fake and the real adapter run this with their respective fixtures.
/// # Panics
/// If a refused file is overwritten or its error kind changes.
pub fn day_store_refusal_contract(store: &dyn DayStore, date: Date, error: DayStoreError) {
    assert_eq!(store.load(date), Err(error));
    assert_eq!(store.save(&Day::new(date, Tuning::default())), Err(error));
    assert_eq!(store.load(date), Err(error));
    assert_eq!(
        store.last_before(date.tomorrow().unwrap_or(date)),
        Err(error)
    );
}
