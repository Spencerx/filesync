use filesync_icons::cloud::CloudUploadIcon;
use leptos::prelude::{ClassAttribute, ElementChild, OnAttribute};
use leptos::{prelude::Children, view};

use filesync_icons::chevron::ChevronLeftIcon;
use js_bindgen::navigate::change_location_to;

use crate::components::toolbar::Toolbar;

#[leptos::component]
pub fn DesktopLayout(children: Children) -> impl leptos::IntoView {
    let children = children();
    view! {
        <header on:click=move |_| change_location_to("/") class="flex justify-between items-center">
            <ChevronLeftIcon />
            <CloudUploadIcon class="animate-pulse" />
        </header>
        <main class="py-3 px-4 overflow-y-scroll">{children}</main>
        <footer class="w-[60%] mx-auto rounded-full fixed bottom-10 left-0 right-0 z-50 border-gray-600 border-[0.25px]  shadow-xl py-0">
            <Toolbar />
        </footer>
    }
}
