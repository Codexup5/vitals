//! The markup of the window: Leptos components that draw what `views` computed.

mod disks;
mod errors;
mod layout;
mod widgets;

use leptos::prelude::*;
use leptos_router::components::{ParentRoute, Redirect, Route, Router, Routes};
use leptos_router::hooks::use_params_map;
use leptos_router::path;

use crate::modules::Module;
use crate::preferences::PreferencesStore;
use widgets::Eyebrow;

#[component]
pub fn App() -> impl IntoView {
    PreferencesStore::provide();
    view! {
        <Router>
            <Routes fallback=|| view! { <layout::Layout><NotFound /></layout::Layout> }>
                <ParentRoute path=path!("") view=layout::LayoutOutlet>
                    <Route path=path!("") view=|| view! { <Redirect path="/errors" /> } />
                    <Route path=path!("errors") view=errors::ErrorsPage />
                    <Route path=path!("disks") view=disks::DisksPage />
                    <Route path=path!(":module") view=ModuleRoute />
                </ParentRoute>
            </Routes>
        </Router>
    }
}

/// A module that has no screen yet, or a section that does not exist.
#[component]
fn ModuleRoute() -> impl IntoView {
    let params = use_params_map();
    move || match params.with(|params| params.get("module").as_deref().and_then(Module::parse)) {
        Some(module) => view! { <ModulePage module /> }.into_any(),
        None => view! { <NotFound /> }.into_any(),
    }
}

#[component]
fn ModulePage(module: Module) -> impl IntoView {
    let store = PreferencesStore::get();
    move || {
        let locale = store.locale();
        let texts = &locale.texts().module_page;
        view! {
            <main class="page">
                <header class="page-heading">
                    <Eyebrow>{texts.eyebrow}</Eyebrow>
                    <h1>{module.title(locale)}</h1>
                </header>
                <p class="muted">{(texts.planned_stage)(module.stage())}</p>
            </main>
        }
    }
}

#[component]
fn NotFound() -> impl IntoView {
    let store = PreferencesStore::get();
    move || {
        let texts = &store.locale().texts().not_found;
        view! {
            <main class="page">
                <h1>{texts.title}</h1>
                <p>{texts.hint}</p>
                <a href="/errors">{texts.home}</a>
            </main>
        }
    }
}
