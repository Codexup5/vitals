//! Small pieces every screen uses.

use leptos::prelude::*;

use crate::views::disks::Tone;
use crate::views::network::Stat;

/// The mono label above a heading: what the section is and where its data comes from
#[component]
pub fn Eyebrow(children: Children) -> impl IntoView {
    view! { <p class="eyebrow">{children()}</p> }
}

/// A soft pill for a state or a total: it always carries its word, the tone only repeats it
#[component]
pub fn Tag(tone: Tone, children: Children) -> impl IntoView {
    view! { <span class="tag" data-tone=tone.code()>{children()}</span> }
}

/// What happened and how to try again, in place of a block that did not load
#[component]
pub fn Failure(
    message: &'static str,
    retry: &'static str,
    on_retry: impl Fn() + 'static,
) -> impl IntoView {
    view! {
        <div role="alert" class="notice">
            <p>{message}</p>
            <button type="button" on:click=move |_| on_retry()>
                {retry}
            </button>
        </div>
    }
}

/// The counters above a screen: a value, its label, a hint; the tone only repeats what the value says
#[component]
pub fn Stats(stats: Vec<Stat>) -> impl IntoView {
    view! {
        <ul class="stats">
            {stats
                .into_iter()
                .map(|stat| {
                    view! {
                        <li class="stat" data-tone=stat.tone.map(Tone::code)>
                            <span class="stat-value">{stat.value}</span>
                            <span>{stat.label}</span>
                            {stat.hint.map(|hint| view! { <span class="muted">{hint}</span> })}
                        </li>
                    }
                })
                .collect_view()}
        </ul>
    }
}
