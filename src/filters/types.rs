// types.rs
#[derive(Debug, Clone, PartialEq)]
pub enum LogicalOp {
    And,
    Or,
    Not,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Operator {
    Eq,
    Gte,
    Lte,
    Gt,
    Lt,
    Ne,
    Like,
    Ilike,
    In,
}

impl Operator {
    // Convertir string de URL a Enum
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "gte" => Some(Self::Gte),
            "lte" => Some(Self::Lte),
            "gt" => Some(Self::Gt),
            "lt" => Some(Self::Lt),
            "ne" => Some(Self::Ne),
            "eq" => Some(Self::Eq),
            "like" => Some(Self::Like),
            "ilike" => Some(Self::Ilike),
            "in" => Some(Self::In),
            _ => None,
        }
    }

    // Convertir Enum a SQL (Postgres specific)
    pub fn as_sql(&self) -> &'static str {
        match self {
            Self::Eq => "=",
            Self::Gte => ">=",
            Self::Lte => "<=",
            Self::Gt => ">",
            Self::Lt => "<",
            Self::Ne => "<>",
            Self::Like => "LIKE",
            Self::Ilike => "ILIKE",
            Self::In => "IN", // Caso especial manejado en el builder
        }
    }
}

#[derive(Debug)]
pub struct FilterCondition {
    pub field: String,
    pub operator: Operator,
    pub value: String,
    pub logic: LogicalOp,
}
