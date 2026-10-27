//! The window frame: the brand, the navigation over the seven modules, the language and theme switches.

use leptos::prelude::*;
use leptos_router::components::{A, Outlet};

use crate::modules::Module;
use crate::preferences::{PreferencesStore, Theme};

#[component]
pub fn LayoutOutlet() -> impl IntoView {
    view! {
        <Layout>
            <Outlet />
        </Layout>
    }
}

#[component]
pub fn Layout(children: Children) -> impl IntoView {
    let store = PreferencesStore::get();
    let texts = move || &store.locale().texts().app;

    view! {
        <div class="layout">
            <aside class="sidebar">
                <p class="brand">
                    <span class="brand-mark" aria-hidden="true"></span>
                    {move || texts().brand}
                </p>
                <nav aria-label=move || texts().modules>
                    <ul class="nav">
                        {Module::ALL
                            .into_iter()
                            .map(|module| {
                                view! {
                                    <li>
                                        <A href=format!("/{}", module.id()) attr:class="nav-link">
                                            {move || module.title(store.locale())}
                                        </A>
                                    </li>
                                }
                            })
                            .collect_view()}
                    </ul>
                </nav>
                <section class="preferences" aria-label=move || texts().preferences>
                    <button
                        type="button"
                        lang=move || store.locale().other().code()
                        aria-label=move || texts().switch_locale_label
                        on:click=move |_| store.update(|preferences| preferences.locale = preferences.locale.other())
                    >
                        {move || texts().switch_locale}
                    </button>
                    <button
                        type="button"
                        on:click=move |_| store.update(|preferences| preferences.theme = preferences.theme.other())
                    >
                        {move || match store.theme() {
                            Theme::Dark => texts().theme_light,
                            Theme::Light => texts().theme_dark,
                        }}
                    </button>
                </section>
            </aside>
            <div class="content">{children()}</div>
        </div>
    }
}
