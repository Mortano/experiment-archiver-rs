use std::fmt::Display;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

/// Is a variable an input or output variable?
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VariableType {
    Input,
    Output,
}

impl Display for VariableType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VariableType::Input => write!(f, "Input"),
            VariableType::Output => write!(f, "Output"),
        }
    }
}

/// The data type of a variable
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DataType {
    /// A numeric variable with a given unit
    Unit(String),
    /// A generic numeric variable
    Number,
    /// A label
    Label,
    /// A generic textual variable
    Text,
    /// A boolean variable
    Bool,
}

impl DataType {
    pub(crate) fn from_str(name: &str) -> Result<Self> {
        match name {
            "Number" => Ok(Self::Number),
            "Label" => Ok(Self::Label),
            "Text" => Ok(Self::Text),
            "Bool" => Ok(Self::Bool),
            other => bail!("Invalid name {other} for DataType"),
        }
    }

    /// Parse the given string as a `GenericValue` matching this `DataType`
    pub(crate) fn parse_str_as_generic_value(&self, value: &str) -> Result<GenericValue> {
        match self {
            DataType::Unit(_) | DataType::Number => {
                let as_float = value
                    .parse()
                    .with_context(|| format!("Failed to parse value {value} as f64"))?;
                Ok(GenericValue::Numeric(as_float))
            }
            DataType::Label | DataType::Text => Ok(GenericValue::String(value.to_string())),
            DataType::Bool => {
                Ok(GenericValue::Bool(value.parse().with_context(|| {
                    format!("Failed to parse value {value} as bool")
                })?))
            }
        }
    }
}

impl Display for DataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataType::Unit(unit) => write!(f, "Unit({unit})"),
            DataType::Number => write!(f, "Number"),
            DataType::Label => write!(f, "Label"),
            DataType::Text => write!(f, "Text"),
            DataType::Bool => write!(f, "Bool"),
        }
    }
}

/// Description of a variable in an experiment
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Variable {
    name: String,
    description: String,
    data_type: DataType,
}

impl Variable {
    pub fn new(name: String, description: String, data_type: DataType) -> Self {
        Self {
            name,
            description,
            data_type,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn data_type(&self) -> &DataType {
        &self.data_type
    }

    pub(crate) fn accepts(&self, generic_value: &GenericValue) -> bool {
        match self.data_type {
            DataType::Unit(_) | DataType::Number => match generic_value {
                GenericValue::Numeric(_) => true,
                _ => false,
            },
            DataType::Label | DataType::Text => match generic_value {
                GenericValue::String(_) => true,
                _ => false,
            },
            DataType::Bool => match generic_value {
                GenericValue::Bool(_) => true,
                _ => false,
            },
        }
    }

    /// Parse the given YAML value as a `Variable`
    #[cfg(feature = "yaml")]
    pub(crate) fn from_yaml(value: &serde_yaml::Value) -> Result<Self> {
        use crate::util::yaml::YamlExt;

        let name = value
            .field_as_str("name")
            .context("Missing 'name' field")?
            .to_string();
        let description = value
            .field_as_str("description")
            .context("Missing 'description' field")?
            .to_string();
        let data_type_value = value
            .get("data_type")
            .context("Missing 'data_type' field")?;
        let data_type = if data_type_value.is_string() {
            DataType::from_str(data_type_value.as_str().unwrap())
                .context("Failed to parse field 'data_type'")?
        } else if data_type_value.is_mapping() {
            let mapping = data_type_value.as_mapping().unwrap();
            let unit_str = mapping
                .get("unit")
                .context("Missing key 'unit' in 'data_type' field")?
                .as_str()
                .context("'unit' field in 'data_type' must be a string")?;
            DataType::Unit(unit_str.to_string())
        } else {
            bail!("'data_type' field must either be a string or a mapping ")
        };

        Ok(Self {
            data_type,
            description,
            name,
        })
    }
}

impl Display for Variable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ({}) - {}",
            self.name, self.description, self.data_type
        )
    }
}

/// Generic value of a variable
#[derive(Debug, PartialEq, Clone)]
pub enum GenericValue {
    Numeric(f64),
    String(String),
    Bool(bool),
}

impl GenericValue {
    pub fn as_f64(&self) -> f64 {
        match self {
            GenericValue::Numeric(val) => *val,
            _ => panic!("self is not of variant Numeric"),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            GenericValue::String(str) => str,
            _ => panic!("self is not of variant String"),
        }
    }

    pub fn as_bool(&self) -> bool {
        match self {
            GenericValue::Bool(b) => *b,
            _ => panic!("self is not of variant Bool"),
        }
    }
}

impl Display for GenericValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GenericValue::Numeric(number) => write!(f, "{number}"),
            GenericValue::String(str) => write!(f, "{str}"),
            GenericValue::Bool(b) => write!(f, "{b}"),
        }
    }
}

/// Specific value of a given variable
#[derive(Debug, PartialEq, Clone)]
pub struct VariableValue<'a> {
    variable: &'a Variable,
    value: GenericValue,
}

impl<'a> VariableValue<'a> {
    /// Create a new `VariableValue` from the given `variable` and the given `value`
    ///
    /// # panics
    ///
    /// If the type of `value` does not match the `data_type` of the variable
    pub fn from_variable(variable: &'a Variable, value: GenericValue) -> Self {
        if !variable.accepts(&value) {
            panic!(
                "Wrong GenericValue for variabl {}. Expected type {:?} but got {value:?}",
                variable.name(),
                variable.data_type()
            );
        }
        Self { value, variable }
    }

    pub fn variable(&self) -> &'a Variable {
        self.variable
    }

    pub fn value(&self) -> &GenericValue {
        &self.value
    }
}

impl Display for VariableValue<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.variable.data_type() {
            DataType::Unit(unit) => write!(f, "{}: {}{}", self.variable.name(), self.value, unit),
            _ => write!(f, "{}: {}", self.variable.name(), self.value),
        }
    }
}
