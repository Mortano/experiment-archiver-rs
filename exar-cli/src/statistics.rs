use std::collections::HashMap;

use exar::{run::ExperimentRun, variable::DataType};
use itertools::Itertools;
use serde::Serialize;
use statrs::statistics::{Data, Distribution, Median};

use crate::util::variable_to_table_display;

#[derive(Debug, Clone, Serialize)]
pub struct AggregatedNumericValue {
    pub average: f64,
    pub median: f64,
    pub standard_deviation: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AggregatedStringValue {
    pub histogram: HashMap<String, usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AggregatedBoolValue {
    pub probability: f64,
}

#[derive(Debug, Clone, Serialize)]
pub enum AggregatedMeasurement {
    Numeric(AggregatedNumericValue),
    String(AggregatedStringValue),
    Bool(AggregatedBoolValue),
}

#[derive(Debug, Clone, Serialize)]
pub struct RunStatistics {
    pub count: usize,
    pub aggregated_measurements: HashMap<String, AggregatedMeasurement>,
}

impl RunStatistics {
    pub fn table_header(&self) -> Vec<String> {
        let mut sorted_names = self.aggregated_measurements.keys().collect_vec();
        sorted_names.sort();

        ["Sample count".to_string()]
            .into_iter()
            .chain(
                sorted_names
                    .iter()
                    .map(|name| {
                        let aggregate = self.aggregated_measurements.get(*name).unwrap();
                        match aggregate {
                            AggregatedMeasurement::Numeric(_) => vec![
                                format!("{name} (mean)"),
                                format!("{name} (median)"),
                                format!("{name} (std. dev.)"),
                            ],
                            AggregatedMeasurement::String(_) => {
                                vec![name.to_string()]
                            }
                            AggregatedMeasurement::Bool(_) => vec![format!("{name} probability")],
                        }
                    })
                    .flatten(),
            )
            .collect_vec()
    }

    pub fn table_row(&self) -> Vec<String> {
        let mut sorted_names = self.aggregated_measurements.keys().collect_vec();
        sorted_names.sort();

        std::iter::once(self.count.to_string())
            .chain(
                sorted_names
                    .iter()
                    .map(|name| {
                        let aggregate = self.aggregated_measurements.get(*name).unwrap();
                        match aggregate {
                            AggregatedMeasurement::Numeric(numeric_aggregate) => vec![
                                numeric_aggregate.average.to_string(),
                                numeric_aggregate.median.to_string(),
                                numeric_aggregate.standard_deviation.to_string(),
                            ],
                            AggregatedMeasurement::String(text_aggregate) => {
                                let mut buckets_descending =
                                    text_aggregate.histogram.iter().collect_vec();
                                buckets_descending.sort_by(|a, b| a.1.cmp(b.1));
                                let combined_histogram = buckets_descending
                                    .into_iter()
                                    .map(|(string, count)| format!("{string}: {count}"))
                                    .join("\n");
                                vec![combined_histogram]
                            }
                            AggregatedMeasurement::Bool(bool_aggregate) => {
                                vec![bool_aggregate.probability.to_string()]
                            }
                        }
                    })
                    .flatten(),
            )
            .collect_vec()
    }
}

pub fn aggregate_runs(runs: &[ExperimentRun<'_>]) -> RunStatistics {
    let output_variables = runs[0]
        .experiment_instance()
        .experiment_version()
        .output_variables();
    let aggregated_measurements = output_variables
        .iter()
        .map(|variable| {
            let description = variable_to_table_display(variable);
            let values = runs.iter().map(|run| {
                let measurement_for_variable = run
                    .measurements()
                    .iter()
                    .find(|m| m.variable() == variable)
                    .unwrap();
                measurement_for_variable.value()
            });

            let aggregate = match variable.data_type() {
                DataType::Unit(_) | DataType::Number => {
                    let data = Data::new(
                        values
                            .filter_map(|v| {
                                let val = v.as_f64();
                                if val.is_nan() {
                                    None
                                } else {
                                    Some(val)
                                }
                            })
                            .collect_vec(),
                    );
                    AggregatedMeasurement::Numeric(AggregatedNumericValue {
                        average: data.mean().unwrap(),
                        median: data.median(),
                        standard_deviation: data.std_dev().unwrap(),
                    })
                }
                DataType::Label | DataType::Text => {
                    let strings = values.map(|v| v.as_str().to_string()).collect_vec();
                    let mut histogram = HashMap::default();
                    for s in strings {
                        if let Some(count) = histogram.get_mut(&s) {
                            *count += 1;
                        } else {
                            histogram.insert(s, 1);
                        }
                    }
                    AggregatedMeasurement::String(AggregatedStringValue { histogram })
                }
                DataType::Bool => {
                    let bools = values.map(|v| v.as_bool()).collect_vec();
                    let probability =
                        bools.iter().filter(|b| **b).count() as f64 / bools.len() as f64;
                    AggregatedMeasurement::Bool(AggregatedBoolValue { probability })
                }
            };

            (description, aggregate)
        })
        .collect();

    RunStatistics {
        count: runs.len(),
        aggregated_measurements,
    }
}
