s/Sexo,/SexoType,/
s/Reaction,/ReactionType,/
s/AmbitoEleccion,/AmbitoEleccionType,/
s/ResourceType,/ResourceTypeType,/

# Include modules
/use super::sql_types::Sexo/ a \
    use crate::types::SexoType;

/use super::sql_types::Reaction/ a \
    use crate::types::ReactionType;

/use super::sql_types::AmbitoEleccion/ a \
    use crate::types::AmbitoEleccionType;

/use super::sql_types::ResourceType/ a \
    use crate::types::ResourceTypeType;
