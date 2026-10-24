//! Declared noteskin controls, shared by the panel and MCP preview requests.
use super::*;
use ddi_noteskin::params::{Kind, Value};
use serde_json::json;
use std::collections::BTreeMap;

impl Panel {
    pub(super) fn set_parameters(
        &mut self,
        patch: BTreeMap<String, Option<Value>>,
    ) -> Result<(), String> {
        let project = self.project.as_ref().ok_or("Load a skin first")?;
        if patch.len() > ddi_noteskin::params::MAX_PARAMETERS {
            return Err("Too many skin parameters".into());
        }
        let mut values = self.preview_parameters.clone();
        for (name, value) in patch {
            let parameter = project
                .parameters
                .get(&name)
                .ok_or_else(|| format!("Unknown skin parameter: {name}"))?;
            match value {
                Some(value) => {
                    let accepted = parameter
                        .accept(&value)
                        .ok_or_else(|| format!("Invalid value for {}", parameter.label))?;
                    if accepted == parameter.default {
                        values.remove(&name);
                    } else {
                        values.insert(name, accepted);
                    }
                }
                None => {
                    values.remove(&name);
                }
            }
        }
        if values == self.preview_parameters {
            return Ok(());
        }
        let previous = std::mem::replace(&mut self.preview_parameters, values);
        if let Err(error) = self.load() {
            self.preview_parameters = previous;
            self.failure = Some(error.clone());
            return Err(error);
        }
        self.checkpoint_dirty = true;
        Ok(())
    }

    /// Each declared parameter's resolved value, by id (the compact form of [`Self::parameter_info`]).
    pub(super) fn parameter_values(&self) -> serde_json::Value {
        let Some(project) = &self.project else {
            return json!({});
        };
        json!(project.parameters.resolve(&self.preview_parameters))
    }

    pub(super) fn parameter_info(&self) -> serde_json::Value {
        let Some(project) = &self.project else {
            return json!([]);
        };
        let resolved = project.parameters.resolve(&self.preview_parameters);
        json!(
            project
                .parameters
                .parameters
                .iter()
                .map(|p| {
                    let (kind, choices, range) = match &p.kind {
                        Kind::Choice(c) | Kind::Color(c) => (
                            if matches!(&p.kind, Kind::Color(_)) {
                                "color"
                            } else {
                                "choice"
                            },
                            json!(
                                c.iter()
                                    .map(|c| json!({"id":c.id,"label":c.label,"skin":c.skin}))
                                    .collect::<Vec<_>>()
                            ),
                            serde_json::Value::Null,
                        ),
                        Kind::Toggle => ("toggle", json!([]), serde_json::Value::Null),
                        Kind::Number {
                            min,
                            max,
                            step,
                            unit,
                        } => (
                            "number",
                            json!([]),
                            json!({"min":min,"max":max,"step":step,"unit":unit}),
                        ),
                    };
                    json!({"id":p.name,"label":p.label,"description":p.description,"kind":kind,
                "default":p.default,"value":resolved.get(&p.name),"choices":choices,"range":range})
                })
                .collect::<Vec<_>>()
        )
    }

    pub(super) fn parameter_controls(&mut self, ui: &mut egui::Ui) {
        let Some(project) = &self.project else {
            return;
        };
        let parameters = project.parameters.clone();
        if parameters.parameters.is_empty() {
            return;
        }
        let resolved = parameters.resolve(&self.preview_parameters);
        let mut patch = BTreeMap::new();
        egui::CollapsingHeader::new("Skin parameters").default_open(true).show(ui, |ui| {
            for parameter in &parameters.parameters {
                let mut value = resolved.get(&parameter.name).unwrap_or(&parameter.default).clone();
                let before = value.clone();
                ui.push_id(&parameter.name, |ui| {
                    ui.horizontal(|ui| {
                        match (&parameter.kind, &mut value) {
                            (Kind::Toggle, Value::Bool(on)) => {
                                ui.checkbox(on, &parameter.label).on_hover_text(&parameter.description);
                            }
                            (Kind::Number { min, max, step, unit }, Value::Number(n)) => {
                                ui.label(&parameter.label).on_hover_text(&parameter.description);
                                ui.add(egui::DragValue::new(n).range(*min..=*max).speed(*step).suffix(unit));
                            }
                            (Kind::Choice(choices) | Kind::Color(choices), Value::Text(id)) => {
                                let label = choices.iter().find(|c| &c.id == id).map(|c| c.label.as_str()).unwrap_or(id);
                                egui::ComboBox::from_id_salt("value").selected_text(label)
                                    .width(125.0).show_ui(ui, |ui| {
                                        for choice in choices { ui.selectable_value(id, choice.id.clone(), &choice.label); }
                                    }).response.on_hover_text(&parameter.description);
                                ui.label(&parameter.label).on_hover_text(&parameter.description);
                            }
                            _ => {}
                        }
                        if ui.add_enabled(value != parameter.default, egui::Button::new("Reset")).clicked() {
                            value = parameter.default.clone();
                        }
                    });
                });
                if value != before { patch.insert(parameter.name.clone(), Some(value)); }
            }
            for warning in &parameters.warnings { ui.colored_label(ui.visuals().warn_fg_color, warning); }
            ui.small("Preview choices are saved with the project. Edit metrics.ini to change exported defaults.");
        });
        if !patch.is_empty()
            && let Err(error) = self.set_parameters(patch)
        {
            self.failure = Some(error);
        }
    }
}
