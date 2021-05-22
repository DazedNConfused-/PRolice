use std::collections::HashMap;
use std::env::temp_dir;
use std::ffi::OsString;
use std::fs::File;
use std::include_str;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use handlebars::Handlebars;
use log::{debug, error, trace};
use serde::{Deserialize, Serialize};

use crate::prolice_error::AnalyzeError;
use crate::scoring::score::Score;
use crate::{nested, prolice_metadata};

pub struct TemplateBuilder {
    owner: String,
    repository: String,
    template_html: &'static str,
    template_data: TemplateData,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TemplateData {
    individual_prs_score: Vec<Score>,
    global_score: Score,
}

impl TemplateData {
    /// Serializes the template's data into JSON format
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(&self).unwrap_or_else(|e| {
            error!("Could not construct JSON for TemplateData [{:#?}].", &self);
            panic!(e);
        })
    }
}

impl TemplateBuilder {
    /// Initializes a [`TemplateBuilder`] with the provided data structures.
    pub fn from(
        owner: &str, repository: &str, individual_prs_score: Vec<Score>, global_score: Score,
    ) -> Self {
        let template_html = include_str!("template/report.hbs");
        let template_data = TemplateData {
            individual_prs_score,
            global_score,
        };

        TemplateBuilder {
            owner: owner.to_string(),
            repository: repository.to_string(),
            template_html,
            template_data,
        }
    }

    /// Builds the report's HTML.
    pub fn build(&self) -> Result<String, AnalyzeError> {
        // initialize report name ---
        let report_template_name = "report";

        // initialize report's inner fields ---
        let owner_name = "owner";
        let repository_name = "repository";
        let template_field_data = "data";

        // build report ---
        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(true);
        handlebars.register_template_string(report_template_name, self.template_html).unwrap();

        let mut template_data: HashMap<&str, String> = HashMap::new();
        template_data.insert(owner_name, self.owner.to_string());
        template_data.insert(repository_name, self.repository.to_string());
        template_data.insert(template_field_data, String::from(&self.template_data.to_json()));

        handlebars.render(report_template_name, &template_data).map_err(|e| {
            trace!("Error = {:?}", e);
            AnalyzeError::TemplateRenderError {
                msg: format!(
                    "Error building report template with name [{}].",
                    report_template_name
                ),
                nested: nested!(e),
            }
        })
    }

    /// Builds the report's HTML and saves it in a temporary folder.
    pub fn build_to_temp_file(&self) -> Result<OsString, AnalyzeError> {
        // first of all, render the output template ---
        let output_template = &self.build()?;

        // with the bytes to write in our possession, attempt to write them in a temp file ---
        let mut temp_dir = temp_dir();
        debug!("Selected temporary directory: [{}]", temp_dir.to_str().unwrap());

        let file_name = format!(
            "{}_{}.html",
            prolice_metadata::package_name(),
            TemplateBuilder::get_epoch_ms()
        );
        debug!("Report will be saved as [{}]", file_name);
        temp_dir.push(file_name);

        let file_path = temp_dir.as_os_str();
        let file_path_str = file_path.to_str().unwrap(); // for logging purposes
        debug!("Saving report in [{}]...", file_path.to_str().unwrap());

        let mut file = File::create(file_path).map_err(|e| {
            trace!("Error = {:?}", e);
            AnalyzeError::TemplateRenderError {
                msg: format!("Error saving report template in path [{}].", file_path_str),
                nested: nested!(e),
            }
        })?;

        file.write_all(output_template.as_bytes()).map_err(|e| {
            trace!("Error = {:?}", e);
            AnalyzeError::TemplateRenderError {
                msg: format!("Error writing into report file [{}].", file_path_str),
                nested: nested!(e),
            }
        })?;

        // if everything has gone correctly, return saved file path ---
        Ok(OsString::from(file_path))
    }

    /// Returns the current System's epoch time.
    /// See more: https://stackoverflow.com/a/65051530
    fn get_epoch_ms() -> u128 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis()
    }
}
