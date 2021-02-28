use std::ffi::CStr;
use std::fs;
use std::env;
use std::boxed::Box;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

use gtk::prelude::*;
use gtk::{
	DialogBuilder,
	Entry,
	EntryBuilder,
	Label,
	LabelBuilder,
	ListBoxBuilder,
	ListBoxRowBuilder,
	Settings,
	Align,
	SelectionMode,
};
use gdk::{EventKey, ModifierType};

use gtk_sys::gtk_rc_get_im_module_file;

const FALSE: i32 = 0;

macro_rules! moved {
	($x:expr, $($capture:ident)*) => {{
		$(let $capture = $capture.to_owned();)*
		$x
	}};
	($($capture:ident)*, $x:expr) => { moved!($x, $($capture)*) };
	($x:expr) => {{ $x }};
}

fn format_key(key: &EventKey) -> String {
	let code = key.get_hardware_keycode();
	let modifiers = key.get_state();
	let mut shortcut = String::new();

	macro_rules! modi {
		($($mask:ident $tok:literal)*) => {
			$(
				if modifiers.contains(ModifierType::$mask) {
					shortcut.push_str($tok);
				}
			)else*
		}
	}

	modi! {
		SUPER_MASK "Super+"
		HYPER_MASK "Hyper+"
		META_MASK "Meta+"
		CONTROL_MASK "Ctrl+"
		MOD1_MASK "Alt+"
		SHIFT_MASK "Shift+"
	}

	if let Some(name) = key.get_keyval().name() {
		shortcut.push_str(name.as_str());
	} else {
		shortcut.push_str("Unknown");
	}

	format!(
		"{shortcut} ({code})",
		shortcut=shortcut, code=code
	)
}

fn list_im_modules() -> Vec<(String, String)> {
	let text = {
		let path = unsafe {
			CStr::from_ptr(gtk_rc_get_im_module_file())
		}.to_str();

		if let Ok(path) = path {
			fs::read_to_string(path).ok()
		} else { None }
	};

	if let Some(text) = text {
		let mut arr: Vec<_> = text.lines().flat_map(
			|line| line.strip_prefix('"').and_then(|line| {
				let mut items = line.splitn(3, "\" \"");
				let id = items.next().map(String::from);
				let name = items.next().map(String::from);
				id.zip(name)
			})
		).collect();
		arr.sort_unstable();
		arr
	} else { Vec::new() }
}

fn current_im_module() -> Option<String> {
	env::var("GTK_IM_MODULE").ok()
		.or_else(|| Settings::get_default()
			.and_then(|s| s.get_property_gtk_im_module())
			.map(|s| s.to_string())
		)
}

#[inline]
fn set_entry_im(input: &Entry, im: Option<&str>) {
	input.reset_im_context();
	input.set_property_im_module(im);
}

fn main() {
	gtk::init().expect("Cannot init GTK.");

	let ime_lock = Arc::new(Mutex::new(current_im_module()));

	let entry = EntryBuilder::new()
		.activates_default(true)
		.editable(true)
		.has_frame(true)
		.has_focus(true)
		.expand(true)
		.hexpand(true)
		.placeholder_text("IME name to switch")
		.width_request(500)
		.build();

	let label = LabelBuilder::new()
		.label("Type input method name and press enter will switch methods.")
		.halign(Align::Start)
		.build();

	let listbox = ListBoxBuilder::new()
		.selection_mode(SelectionMode::Single)
		.build();

	let dialog = DialogBuilder::new()
		.decorated(false)
		.use_header_bar(FALSE)
		.resizable(false)
		.skip_taskbar_hint(true)
		.build();

	{
		let content = dialog.get_content_area();
		content.add(&label);
		content.add(&entry);
		content.add(&listbox);
	}

	listbox.set_sort_func(Some(Box::new(|a, b| {
		let a = a.get_widget_name().to_string();
		let b = b.get_widget_name().to_string();
		a.cmp(&b) as i32
	})));

	listbox.set_filter_func(Some(Box::new(moved!(entry listbox, move |row| {
		let text = entry.get_text().to_string();
		let text = text.trim();
		let name = row.get_widget_name().to_string();

		let result = text.is_empty() || name.contains(text);

		if row.get_activatable() != result {
			row.set_activatable(result);
		}

		if row.get_selectable() != result {
			if row.is_selected() && !result {
				listbox.unselect_row(row);
			}

			row.set_selectable(result);
		}

		result
	}))));

	let im_modules = Arc::new({
		let im_modules = list_im_modules();
		let im = ime_lock.lock().unwrap().clone()
			.or_else(|| entry.get_property_im_module().map(|s| s.to_string()))
			.unwrap_or_default();

		im_modules.iter().map(|(id, description)| {
			let row = ListBoxRowBuilder::new()
				.child(&Label::new(Some(
					format!("{}: {}", id, description).as_str()
				)))
				.selectable(true)
				.activatable(true)
				.parent(&listbox)
				.name(id)
				.build();

			if im == *id {
				listbox.select_row(Some(&row));
			}

			(id.clone(), row)
		}).collect::<HashMap<String, _>>()
	});

	moved!(ime_lock label, entry.connect_property_im_module_notify(move |input| {
		let im = input.get_property_im_module()
			.map(|s| s.to_string())
			.unwrap_or_default();

		label.set_text(format!("IME has been set to {}", im).as_str());

		*ime_lock.lock().unwrap() = Some(im);
	}));

	moved!(entry, listbox.connect_row_activated(move |_, row| {
		let im = row.get_widget_name().to_string().trim().to_string();

		if let Some(ime) = ime_lock.lock().unwrap().clone() {
			if ime == im {
				return;
			}
		}

		set_entry_im(
			&entry,
			Some(im.as_str()),
		);

		entry.grab_focus();
	}));

	moved!(listbox, entry.connect_activate(move |input| {
		let im = input.get_text().to_string().trim().to_owned();

		if let Some(row) = im_modules.get(&im) {
			row.activate();
		} else if let Some(row) = listbox.get_row_at_y(0) {
			row.activate();
		} else {
			return
		}

		input.set_text("");
	}));

	moved!(label, entry.connect_key_press_event(move |_, key| {
		label.set_text(format_key(&key).as_str());

		Inhibit(false)
	}));

	moved!(listbox, entry.connect_property_text_notify(move |_| {
		listbox.invalidate_filter();
	}));

	dialog.show_all();
	dialog.run();
}

