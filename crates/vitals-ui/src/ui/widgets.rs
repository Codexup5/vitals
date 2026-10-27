//! Small pieces every screen uses.

use leptos::prelude::*;

use crate::views::disks::Tone;

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
