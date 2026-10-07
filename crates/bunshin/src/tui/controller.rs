//! Translate screen effects to port calls before accepting another key.
use bunshin_core::{
    Now,
    day::store::DayStore,
    screen::{Effect, MainScreen, ScreenKey},
};

/// Finish every requested save and record its result before the caller reads another key.
pub(super) fn process_key(
    screen: MainScreen,
    key: ScreenKey,
    now: Now,
    store: &dyn DayStore,
    cancel: impl FnMut(),
) -> MainScreen {
    let (screen, effects) = screen.update(key, now);
    process_effects(screen, effects, now, store, cancel)
}

/// Recover pending owner rows before the terminal's error path drops the screen.
pub(super) fn cancel_after_error(
    screen: MainScreen,
    now: Now,
    store: &dyn DayStore,
    mut cancel: impl FnMut(),
) -> MainScreen {
    cancel();
    process_key(screen, ScreenKey::Interrupt, now, store, || {})
}

pub(super) fn process_effects(
    mut screen: MainScreen,
    effects: Vec<Effect>,
    now: Now,
    store: &dyn DayStore,
    mut cancel: impl FnMut(),
) -> MainScreen {
    let mut queue = std::collections::VecDeque::from(effects);
    while let Some(effect) = queue.pop_front() {
        match effect {
            Effect::Save => {
                let result = store.save(screen.day());
                let notice = result
                    .err()
                    .map_or_else(String::new, crate::wording::save_failure);
                screen = screen.record_save_result(result, now.instant, &notice);
            }
            Effect::Quit => screen = screen.complete_quit(),
            Effect::CancelModel => cancel(),
            Effect::ChatNotice(notice) => {
                let text = crate::wording::chat_notice(notice);
                screen.record_chat_notice(notice, &text, now.instant);
            }
        }
    }
    screen
}
