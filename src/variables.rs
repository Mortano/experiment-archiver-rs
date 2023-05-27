use std::borrow::Cow;

use anyhow::{bail, Context, Result};
use postgres::{GenericClient, Row};

use crate::gen_unique_id;

/// Template for a variable definition that is part of an experiment
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct VariableTemplate {
    name: Cow<'static, str>,
    description: Cow<'static, str>,
    unit: Cow<'static, str>,
}

impl VariableTemplate {
    /// Creates a new template for a variable with the given name, description and unit
    pub const fn new(
        name: Cow<'static, str>,
        description: Cow<'static, str>,
        unit: Cow<'static, str>,
    ) -> Self {
        Self {
            name,
            description,
            unit,
        }
    }

    /// The name of this variable template
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The description of this variable template
    pub fn description(&self) -> &str {
        &self.description
    }

    /// The unit of this variable template
    pub fn unit(&self) -> &str {
        &self.unit
    }

    /// Insert this VariableTemplate into the database and return the corresponding variable
    pub(crate) fn insert_into_db<C: GenericClient>(&self, client: &mut C) -> Result<Variable> {
        let variable_id = gen_unique_id();
        let changed_rows = client
            .execute(
                "INSERT INTO variables VALUES ($1, $2, $3, $4)",
                &[&variable_id, &self.name, &self.description, &self.unit],
            )
            .context("Failed to execute INSERT statement")?;
        if changed_rows != 1 {
            bail!("Unexpected number of affected rows ({})", changed_rows);
        }
        Ok(Variable {
            id: variable_id,
            template: self.clone(),
        })
    }
}

/// Variable definition after inserting into the DB or fetching from the DB
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Variable {
    id: String,
    template: VariableTemplate,
}

impl Variable {
    /// Access the `VariableTemplate` for this variable, which defines the name, description etc.
    pub fn template(&self) -> &VariableTemplate {
        &self.template
    }

    /// Access the ID of this Variable
    pub fn id(&self) -> &str {
        &self.id
    }
}

impl TryFrom<&'_ Row> for Variable {
    type Error = anyhow::Error;

    fn try_from(value: &'_ Row) -> std::result::Result<Self, Self::Error> {
        let id = value.try_get("id").context("id field not found in row")?;
        let name: String = value
            .try_get("name")
            .context("name field not found in row")?;
        let description: String = value
            .try_get("description")
            .context("description field not found in row")?;
        let unit: String = value
            .try_get("unit")
            .context("unit field not found in row")?;
        Ok(Self {
            id,
            template: VariableTemplate {
                name: name.into(),
                description: description.into(),
                unit: unit.into(),
            },
        })
    }
}
