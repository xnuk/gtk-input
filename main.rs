use std::ffi::CStr;
use std::fs;
use std::env;

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
			)*
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

fn set_entry_im(input: &Entry, im: Option<String>) -> String {
	input.reset_im_context();

	let im = im.unwrap_or_else(|| {
		let a = input.get_text().to_string();
		input.set_text("");
		a
	});

	let trimmed = im.trim();

	input.set_property_im_module(Some(trimmed));

	trimmed.to_string()
}

fn main() {
	gtk::init().expect("Cannot init GTK.");

	let entry = EntryBuilder::new()
		.activates_default(true)
		.editable(true)
		.has_frame(true)
		.has_focus(true)
		.expand(true)
		.hexpand(true)
		.placeholder_text("IME name to switch")
		.width_request(500)
		.build()
	;

	let dialog = DialogBuilder::new()
		.decorated(false)
		.use_header_bar(FALSE)
		.resizable(false)
		.skip_taskbar_hint(true)
		.build()
	;

	let label = LabelBuilder::new()
		.label("Type input method name and press enter will switch methods.")
		.halign(Align::Start)
		.build()
	;

	let listbox = ListBoxBuilder::new()
		.selection_mode(SelectionMode::Single)
		.build()
	;

	let im = current_im_module()
		.or_else(|| entry.get_property_im_module().map(|s| s.to_string()))
		.unwrap_or_else(|| String::new());

	let im_modules = list_im_modules();

	let rows: Vec<String> = im_modules.iter().enumerate().map(|(index, (id, description))| {
		let row = ListBoxRowBuilder::new()
			.child(
				&Label::new(Some(
					format!("{}: {}", id, description).as_str()
				))
			)
			.selectable(true)
			.build();

		listbox.insert(&row, index as i32);

		if im == *id {
			listbox.select_row(Some(&row));
		}

		id.clone()
	}).collect();

	let content = dialog.get_content_area();
	content.add(&label);
	content.add(&entry);
	content.add(&listbox);


	let rows_cloned = rows.clone();
	let entry_cloned = entry.clone();

	listbox.connect_row_selected(move |_, row| {
		let im = row.and_then(|row| rows_cloned.get(row.get_index() as usize));
		if let Some(im) = im {
			set_entry_im(&entry_cloned, Some(im.clone()));
		}
	});

	entry.connect_activate(move |input| {
		let im = set_entry_im(&input, None);

		if let Ok(index) = rows.binary_search(&im) {
			if let Some(row) = listbox.get_row_at_index(index as i32) {
				listbox.select_row(Some(&row))
			}
		}
	});

	entry.connect_key_press_event(move |_, key| {
		let a = format_key(&key);
		label.set_text(a.as_str());
		Inhibit(false)
	});

	dialog.show_all();
	dialog.run();
}

