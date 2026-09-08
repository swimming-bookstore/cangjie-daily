use cj_core::{
    elect_lesson, elect_more, format_day_key, parse_table, radical_for, registered_pool, Glyph,
    Table, SESSION_N,
};
use leptos::ev;
use leptos::html;
use leptos::prelude::*;
use std::collections::HashSet;
use std::time::Duration;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}

async fn load_table() -> Result<Table, String> {
    let window = web_sys::window().ok_or("no window")?;
    let resp_val = JsFuture::from(window.fetch_with_str("cangjie.json"))
        .await
        .map_err(|e| format!("fetch: {e:?}"))?;
    let resp: web_sys::Response = resp_val
        .dyn_into()
        .map_err(|_| "fetch did not return a Response".to_string())?;
    if !resp.ok() {
        return Err(format!("cangjie.json HTTP {}", resp.status()));
    }
    let text_val = JsFuture::from(resp.text().map_err(|e| format!("{e:?}"))?)
        .await
        .map_err(|e| format!("text: {e:?}"))?;
    let raw = text_val.as_string().ok_or("body was not a string")?;
    parse_table(&raw).map_err(|e| e.to_string())
}

fn today_parts() -> (i32, u32, u32) {
    let d = js_sys::Date::new_0();
    (
        d.get_full_year() as i32,
        d.get_month() as u32 + 1,
        d.get_date() as u32,
    )
}

fn key_label(letter: char, radicals: &std::collections::HashMap<String, String>) -> String {
    radical_for(letter, radicals)
        .map(|s| s.to_string())
        .unwrap_or_else(|| letter.to_ascii_uppercase().to_string())
}

fn format_code(code: &str, radicals: &std::collections::HashMap<String, String>) -> String {
    code.chars()
        .map(|ch| key_label(ch, radicals))
        .collect::<Vec<_>>()
        .join(" ")
}

fn empty_slots() -> Vec<String> {
    vec![String::new(); 5]
}

fn slots_code(slots: &[String]) -> String {
    let last = slots
        .iter()
        .rposition(|s| !s.is_empty())
        .map(|i| i + 1)
        .unwrap_or(0);
    let mut out = String::new();
    for s in slots.iter().take(last) {
        if s.is_empty() {
            out.push('\0');
        } else {
            out.push_str(s);
        }
    }
    out
}

fn slot_at(over: bool, finished: bool, code: &str, buffers: &[Vec<String>], i: usize, s: usize) -> String {
    if over || finished {
        code.chars().nth(s).map(|c| c.to_string()).unwrap_or_default()
    } else {
        buffers
            .get(i)
            .and_then(|row| row.get(s))
            .cloned()
            .unwrap_or_default()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    Play,
    Pause,
    Over,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Flash {
    Ok,
    Bad,
}

#[component]
fn App() -> impl IntoView {
    let table = LocalResource::new(|| async { load_table().await });

    view! {
        <div class="min-h-screen bg-white px-3 py-4 text-black sm:px-4 sm:py-6">
            <Suspense fallback=move || {
                view! {
                    <p class="mx-auto max-w-md p-8 text-center text-neutral-500">
                        "Opening…"
                    </p>
                }
            }>
                {move || {
                    match table.get() {
                        None => None,
                        Some(Ok(t)) => Some(view! { <Game table=t /> }.into_any()),
                        Some(Err(e)) => Some(
                            view! {
                                <p class="mx-auto max-w-lg p-6 text-black">
                                    "Could not load cangjie.json: " {e}
                                </p>
                            }
                            .into_any(),
                        ),
                    }
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn Game(table: Table) -> impl IntoView {
    let pool_vec = registered_pool(&table.chars);
    let (y, m, d) = today_parts();
    let day_key = format_day_key(y, m, d);
    let first = elect_lesson(&pool_vec, &day_key, SESSION_N);
    let mut used0 = HashSet::new();
    for g in &first {
        used0.insert(g.h.clone());
    }

    let screen = RwSignal::new(Screen::Play);
    let card_n = RwSignal::new(0usize);
    let buffers = RwSignal::new(vec![empty_slots(); SESSION_N]);
    let carets = RwSignal::new(vec![0usize; SESSION_N]);
    let done = RwSignal::new(vec![false; SESSION_N]);
    let hint_on = RwSignal::new(vec![false; SESSION_N]);
    let flash = RwSignal::new(vec![None; SESSION_N]);
    let extra_round = RwSignal::new(0u32);
    let used = RwSignal::new(used0);
    let lesson = RwSignal::new(first);
    let list_offset = RwSignal::new(0usize);

    let pool = StoredValue::new(pool_vec);
    let radicals = StoredValue::new(table.radicals.clone());
    let day_key = StoredValue::new(day_key);

    let fresh_rows = move |n: usize| {
        card_n.set(0);
        buffers.set(vec![empty_slots(); n]);
        carets.set(vec![0usize; n]);
        done.set(vec![false; n]);
        hint_on.set(vec![false; n]);
        flash.set(vec![None; n]);
    };

    let more_ten = move || {
        let r = extra_round.get() + 1;
        extra_round.set(r);
        let next = pool.with_value(|p| {
            day_key.with_value(|k| used.with(|u| elect_more(p, k, SESSION_N, r, u)))
        });
        if next.is_empty() {
            return;
        }
        list_offset.set(used.with(|u| u.len()));
        used.update(|u| {
            for g in &next {
                u.insert(g.h.clone());
            }
        });
        let n = next.len();
        lesson.set(next);
        fresh_rows(n);
        screen.set(Screen::Play);
    };

    let finish = move || {
        screen.set(Screen::Over);
    };

    let accept = move || {
        let i = card_n.get();
        done.update(|d| {
            if let Some(slot) = d.get_mut(i) {
                *slot = true;
            }
        });
        if done.with(|d| d.iter().all(|ok| *ok)) {
            finish();
        } else {
            let n = lesson.with(|ls| ls.len()).max(1);
            let next = done.with(|d| {
                (1..=n)
                    .map(|k| (i + k) % n)
                    .find(|&j| !d.get(j).copied().unwrap_or(true))
                    .unwrap_or(i)
            });
            card_n.set(next);
        }
    };

    let check_answer = move |i: usize| {
        if screen.get() != Screen::Play {
            return;
        }
        if done.with(|d| d.get(i).copied().unwrap_or(true)) {
            return;
        }
        card_n.set(i);
        let Some(card) = lesson.with(|ls| ls.get(i).cloned()) else {
            return;
        };
        let buf = buffers.with(|b| {
            b.get(i)
                .map(|slots| slots_code(slots))
                .unwrap_or_default()
        });
        let mark = if buf == card.c {
            Flash::Ok
        } else {
            Flash::Bad
        };
        flash.update(|f| {
            if let Some(slot) = f.get_mut(i) {
                *slot = Some(mark);
            }
        });
        set_timeout(
            move || {
                flash.update(|f| {
                    if let Some(slot) = f.get_mut(i) {
                        *slot = None;
                    }
                });
            },
            Duration::from_millis(500),
        );
        if mark == Flash::Ok {
            accept();
        } else {
            carets.update(|c| {
                if let Some(cur) = c.get_mut(i) {
                    *cur = 0;
                }
            });
        }
    };

    let toggle_hint = move |i: usize| {
        if screen.get() != Screen::Play {
            return;
        }
        if done.with(|d| d.get(i).copied().unwrap_or(true)) {
            return;
        }
        card_n.set(i);
        hint_on.update(|h| {
            if let Some(slot) = h.get_mut(i) {
                *slot = !*slot;
            }
        });
    };

    let focus_row = move |i: usize, slot: Option<usize>| {
        if screen.get() != Screen::Play {
            return;
        }
        if done.with(|d| d.get(i).copied().unwrap_or(true)) {
            return;
        }
        card_n.set(i);
        if let Some(s) = slot {
            carets.update(|c| {
                if let Some(cur) = c.get_mut(i) {
                    *cur = s.min(4);
                }
            });
        }
    };

    let type_box = NodeRef::<html::Input>::new();

    let keep_kb = move || {
        if screen.get() != Screen::Play {
            if let Some(el) = type_box.get() {
                let _ = el.blur();
            }
            return;
        }
        if let Some(el) = type_box.get() {
            let _ = el.focus();
            el.set_value(" ");
            let _ = el.set_selection_range(1, 1);
        }
    };

    let push_char = move |ch: char| {
        if screen.get() != Screen::Play {
            return;
        }
        let i = card_n.get();
        if done.with(|d| d.get(i).copied().unwrap_or(true)) {
            return;
        }
        let ch = if ch.is_ascii_uppercase() {
            ch.to_ascii_lowercase()
        } else {
            ch
        };
        let caret = carets.with(|c| c.get(i).copied().unwrap_or(0)).min(4);
        buffers.update(|b| {
            if let Some(row) = b.get_mut(i) {
                if let Some(cell) = row.get_mut(caret) {
                    *cell = ch.to_string();
                }
            }
        });
        if caret < 4 {
            carets.update(|c| {
                if let Some(cur) = c.get_mut(i) {
                    *cur = caret + 1;
                }
            });
        }
    };

    let erase = move || {
        if screen.get() != Screen::Play {
            return;
        }
        let i = card_n.get();
        if done.with(|d| d.get(i).copied().unwrap_or(true)) {
            return;
        }
        let caret = carets.with(|c| c.get(i).copied().unwrap_or(0));
        let mut step_back = false;
        buffers.update(|b| {
            if let Some(row) = b.get_mut(i) {
                if row.get(caret).map(|s| !s.is_empty()).unwrap_or(false) {
                    if let Some(cell) = row.get_mut(caret) {
                        cell.clear();
                    }
                } else if caret > 0 {
                    if let Some(cell) = row.get_mut(caret - 1) {
                        cell.clear();
                    }
                    step_back = true;
                }
            }
        });
        if step_back {
            carets.update(|c| {
                if let Some(cur) = c.get_mut(i) {
                    *cur = caret - 1;
                }
            });
        }
    };

    let on_key = move |ev: web_sys::KeyboardEvent, from_box: bool| {
        let key = ev.key();
        let special = key == "Backspace"
            || key == " "
            || key == "Enter"
            || key == "Escape"
            || key == "ArrowLeft"
            || key == "ArrowRight";
        // Do not preventDefault letters in the hidden input: iOS often skips
        // keydown and only fires `input`. Desktop letters go through `input`.
        if special || (!from_box && key.chars().count() == 1) {
            ev.prevent_default();
        }
        let key = key.as_str();
        match screen.get() {
            Screen::Over => {
                if key == "Enter" {
                    more_ten();
                }
            }
            Screen::Pause => {
                if key == "Escape" {
                    screen.set(Screen::Play);
                }
            }
            Screen::Play => {
                if key == "Escape" {
                    screen.set(Screen::Pause);
                } else if key == "Enter" || key == " " {
                    check_answer(card_n.get());
                    set_timeout(move || keep_kb(), Duration::from_millis(10));
                } else if key == "ArrowLeft" {
                    let i = card_n.get();
                    carets.update(|c| {
                        if let Some(cur) = c.get_mut(i) {
                            *cur = cur.saturating_sub(1);
                        }
                    });
                } else if key == "ArrowRight" {
                    let i = card_n.get();
                    carets.update(|c| {
                        if let Some(cur) = c.get_mut(i) {
                            *cur = (*cur + 1).min(4);
                        }
                    });
                } else if key == "Backspace" {
                    erase();
                } else if !from_box && key.chars().count() == 1 {
                    if let Some(ch) = key.chars().next() {
                        if !ch.is_control() {
                            push_char(ch);
                        }
                    }
                }
            }
        }
    };

    let handle = window_event_listener(ev::keydown, move |ev: web_sys::KeyboardEvent| {
        if ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
            .is_some()
        {
            return;
        }
        on_key(ev, false);
    });
    on_cleanup(move || handle.remove());

    Effect::new(move |_| {
        let _ = (card_n.get(), screen.get());
        keep_kb();
    });

    view! {
        <section class="relative mx-auto max-w-2xl overflow-x-hidden" on:click=move |_| keep_kb()>
            <input
                node_ref=type_box
                class="cj-type"
                type="text"
                inputmode="text"
                lang="en"
                autocomplete="off"
                autocapitalize="none"
                spellcheck="false"
                enterkeyhint="done"
                aria-label="Cangjie code"
                on:blur=move |_| {
                    if screen.get() == Screen::Play {
                        set_timeout(move || keep_kb(), Duration::from_millis(0));
                    }
                }
                on:keydown=move |ev| on_key(ev, true)
                on:input=move |_| {
                    let Some(el) = type_box.get() else {
                        return;
                    };
                    let v = el.value();
                    if v.is_empty() {
                        erase();
                    } else {
                        for ch in v.chars() {
                            if ch.is_ascii_alphabetic() {
                                push_char(ch);
                            }
                        }
                    }
                    el.set_value(" ");
                    let _ = el.set_selection_range(1, 1);
                }
            />
            <div class="mx-auto w-full max-w-full sm:w-fit">
            <PlayList
                screen=screen
                card_n=card_n
                buffers=buffers
                done=done
                hint_on=hint_on
                radicals=radicals
                lesson=lesson
                carets=carets
                flash=flash
                list_offset=list_offset
                on_check=Callback::new(move |i: usize| {
                    check_answer(i);
                    keep_kb();
                })
                on_hint=Callback::new(move |i: usize| {
                    toggle_hint(i);
                    keep_kb();
                })
                on_focus=Callback::new(move |(i, slot): (usize, Option<usize>)| {
                    focus_row(i, slot);
                    keep_kb();
                })
            />
            <div class="mt-6 flex justify-center">
                <button
                    class="border border-black bg-black px-6 py-2 text-white"
                    on:pointerdown=move |ev| ev.prevent_default()
                    on:click=move |_| more_ten()
                >
                    "10 more"
                </button>
            </div>
            </div>
            <Show when=move || screen.get() == Screen::Pause>
                <div class="absolute inset-0 flex items-center justify-center bg-white/90">
                    <p class="font-serif text-4xl">"Paused"</p>
                </div>
            </Show>
        </section>
    }
}

#[component]
fn PlayList(
    screen: RwSignal<Screen>,
    card_n: RwSignal<usize>,
    buffers: RwSignal<Vec<Vec<String>>>,
    carets: RwSignal<Vec<usize>>,
    done: RwSignal<Vec<bool>>,
    hint_on: RwSignal<Vec<bool>>,
    radicals: StoredValue<std::collections::HashMap<String, String>>,
    lesson: RwSignal<Vec<Glyph>>,
    flash: RwSignal<Vec<Option<Flash>>>,
    list_offset: RwSignal<usize>,
    on_check: Callback<usize>,
    on_hint: Callback<usize>,
    on_focus: Callback<(usize, Option<usize>)>,
) -> impl IntoView {
    view! {
        <div class="space-y-2">
            {move || {
                lesson.with(|ls| {
                    ls.iter()
                        .enumerate()
                        .map(|(i, g)| {
                            let han = g.h.clone();
                            let code = g.c.clone();
                            view! {
                                <div
                                    class=move || {
                                        match flash.with(|f| f.get(i).copied().flatten()) {
                                            Some(Flash::Bad) => "cj-row row-bad",
                                            _ => "cj-row",
                                        }
                                    }
                                    on:click=move |_| on_focus.run((i, None))
                                >
                                    <span class="w-6 shrink-0 text-center text-sm text-neutral-500 sm:w-8">
                                        {move || list_offset.get() + i + 1}
                                    </span>
                                    <span class="w-10 shrink-0 text-center font-serif text-3xl sm:w-12">
                                        {han}
                                    </span>
                                    <div class="cj-boxes">
                                        {(0..5)
                                            .map(|s| {
                                                let code = code.clone();
                                                view! {
                                                    <CodeSlot
                                                        s=s
                                                        i=i
                                                        code=code
                                                        screen=screen
                                                        card_n=card_n
                                                        carets=carets
                                                        buffers=buffers
                                                        done=done
                                                        flash=flash
                                                        radicals=radicals
                                                        on_focus=on_focus
                                                    />
                                                }
                                            })
                                            .collect_view()}
                                    </div>
                                    <div class="cj-acts">
                                    <Show when=move || {
                                        screen.get() != Screen::Over
                                            && !done.with(|d| d.get(i).copied().unwrap_or(false))
                                    }>
                                        <button
                                            class="h-8 w-16 shrink-0 border border-black bg-black text-sm text-white"
                                            on:pointerdown=move |ev| ev.prevent_default()
                                            on:click=move |ev| {
                                                ev.stop_propagation();
                                                on_check.run(i);
                                            }
                                        >
                                            "Check"
                                        </button>
                                        <button
                                            class=move || {
                                                if hint_on.with(|h| h.get(i).copied().unwrap_or(false)) {
                                                    "h-8 w-14 shrink-0 border border-black text-sm"
                                                } else {
                                                    "h-8 w-14 shrink-0 border border-neutral-400 text-sm text-neutral-500"
                                                }
                                            }
                                            on:pointerdown=move |ev| ev.prevent_default()
                                            on:click=move |ev| {
                                                ev.stop_propagation();
                                                on_hint.run(i);
                                            }
                                        >
                                            "Hint"
                                        </button>
                                    </Show>
                                    <span class=move || {
                                        let f = flash.with(|fl| fl.get(i).copied().flatten());
                                        if f == Some(Flash::Ok)
                                            || done.with(|d| d.get(i).copied().unwrap_or(false))
                                        {
                                            "w-6 shrink-0 text-center text-sm text-emerald-700"
                                        } else if f == Some(Flash::Bad) {
                                            "w-6 shrink-0 text-center text-sm text-red-700"
                                        } else {
                                            "w-6 shrink-0 text-center text-sm"
                                        }
                                    }>
                                        {move || {
                                            let f = flash.with(|fl| fl.get(i).copied().flatten());
                                            if f == Some(Flash::Ok)
                                                || done.with(|d| d.get(i).copied().unwrap_or(false))
                                            {
                                                "OK"
                                            } else if f == Some(Flash::Bad) {
                                                "X"
                                            } else {
                                                ""
                                            }
                                        }}
                                    </span>
                                    <span class="cj-hint-line min-w-0 font-serif text-lg leading-8 text-neutral-600 empty:hidden">
                                        {
                                            let code = code.clone();
                                            move || {
                                                let show = screen.get() == Screen::Over
                                                    || hint_on.with(|h| h.get(i).copied().unwrap_or(false));
                                                if show {
                                                    radicals.with_value(|r| format_code(&code, r))
                                                } else {
                                                    String::new()
                                                }
                                            }
                                        }
                                    </span>
                                    </div>
                                </div>
                            }
                        })
                        .collect_view()
                })
            }}
        </div>
    }
}

fn slot_label(ch: char, radicals: &std::collections::HashMap<String, String>) -> String {
    if ch.is_ascii_lowercase() {
        key_label(ch, radicals)
    } else {
        ch.to_string()
    }
}

#[component]
fn CodeSlot(
    s: usize,
    i: usize,
    code: String,
    screen: RwSignal<Screen>,
    card_n: RwSignal<usize>,
    carets: RwSignal<Vec<usize>>,
    buffers: RwSignal<Vec<Vec<String>>>,
    done: RwSignal<Vec<bool>>,
    flash: RwSignal<Vec<Option<Flash>>>,
    radicals: StoredValue<std::collections::HashMap<String, String>>,
    on_focus: Callback<(usize, Option<usize>)>,
) -> impl IntoView {
    view! {
        <div
            class=move || {
                let over = screen.get() == Screen::Over;
                let finished = done.with(|d| d.get(i).copied().unwrap_or(false));
                let current = !over && !finished && card_n.get() == i;
                let caret = carets.with(|c| c.get(i).copied().unwrap_or(0));
                let mark = flash.with(|f| f.get(i).copied().flatten());
                let border = if mark == Some(Flash::Ok) || finished {
                    "border-emerald-600 bg-emerald-50 text-emerald-800"
                } else if mark == Some(Flash::Bad) {
                    "border-red-600 bg-red-50 text-red-800"
                } else if current && caret == s {
                    "border-black bg-white"
                } else {
                    "border-neutral-300 bg-white"
                };
                format!(
                    "cj-slot flex items-center justify-center border font-serif {border}"
                )
            }
            on:click=move |ev| {
                ev.stop_propagation();
                on_focus.run((i, Some(s)));
            }
        >
            {move || {
                let over = screen.get() == Screen::Over;
                let finished = done.with(|d| d.get(i).copied().unwrap_or(false));
                let cell = slot_at(over, finished, &code, &buffers.get(), i, s);
                cell.chars()
                    .next()
                    .map(|ch| radicals.with_value(|r| slot_label(ch, r)))
                    .unwrap_or_default()
            }}
        </div>
    }
}
