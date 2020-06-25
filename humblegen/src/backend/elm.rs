//! Elm code generator.

use crate::{ast, Artifact, LibError, Spec};
use anyhow::{Context, Result};
use std::io::{self, BufWriter};
use inflector::cases::camelcase::to_camel_case;
use itertools::Itertools;
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

const BACKEND_NAME: &str = "elm";

pub(crate) struct IndentWriter {
    indent: usize,
    outstream : Box<dyn io::Write>,
}

impl IndentWriter {
    pub(crate) fn for_file(outdir : &Path, filename :&str) -> Result<Self, LibError> {
        let data_path = { let mut p = PathBuf::from(outdir); p.push(filename); p };

        let outfile = File::create(&data_path).map_err(LibError::IoError)?;
        let outstream = BufWriter::new(outfile);

        Ok(Self { outstream: Box::new(outstream), indent: 0 })
    }

    fn kill_indent(&mut self) {
        self.indent = 0;
    }

    fn increase_indent(&mut self) -> String {
        self.indent += 1;
        self.newline()
    }

    fn decrease_indent(&mut self) -> String {
        self.indent -= 1;
        self.newline()
    }

    fn tabs(&self) -> String {
        "    ".repeat(self.indent)
    } 

    fn newline(&self) -> String {
        format!("\n{}", self.tabs())
    } 

    fn start_line(&mut self) -> Result<&mut dyn io::Write, LibError> {
        write!(self.outstream, "\n{}", self.tabs())?;
        Ok(&mut self.outstream)
    }

    fn handle(&mut self) -> &mut dyn io::Write {
        &mut self.outstream
    }

    fn empty_lines(&mut self, num : usize) -> Result<(), LibError> {
        write!(self.outstream, "{}", "\n".repeat(num))?;
        Ok(())
    }
}

/// Generate elm code for a docstring.
///
/// If not present, generates an empty string.
fn generate_doc_comment(doc_comment: &Option<String>) -> String {
    match doc_comment {
        Some(ref ds) => format!("{{-| {ds}\n-}}\n", ds = ds),
        None => "".to_owned(),
    }
}

// TODO: Elm does not allow documentation on members, so the docs need to be converted to markdown
//       lists instead. This is true for `type alias` struct fields as well as enum variants.

pub(crate) fn generate_struct_def(def: &ast::StructDef, file :&mut IndentWriter) -> Result<(), LibError> {
    file.kill_indent();

    write!(file.start_line()?, "{doc_comment}type alias {name} =",
        doc_comment = generate_doc_comment(&def.doc_comment),
        name = def.name)?;

    generate_struct_fields(&def.fields, file)?;

    file.empty_lines(2)?;

    Ok(())
}

pub(crate) fn generate_struct_fields(fields: &ast::StructFields, file :&mut IndentWriter) -> Result<(), LibError> {
        
    file.increase_indent();

    for (idx, field) in fields.iter().enumerate() {
        let first = idx == 0;
        generate_struct_field(field, first, file)?;
    }
    
    write!(file.start_line()?, "}}")?;

    file.decrease_indent();

    Ok(())
}


fn generate_struct_field(field: &ast::FieldNode, first : bool, file :&mut IndentWriter) -> Result<(), LibError> {
    write!(file.start_line()?, "{delimiter}{name}: {ty}",
        delimiter = if first { "{ " } else { ", " }, 
        name = field_name(&field.pair.name),
        ty = generate_type_ident(&field.pair.type_ident)
    )?;

    Ok(())
}

/// Generate elm code for an enum definition.
pub(crate) fn generate_enum_def(def: &ast::EnumDef, file :&mut IndentWriter) -> Result<(), LibError> {
    file.kill_indent();

    write!(file.start_line()?, "{doc_comment}type {name}",
         doc_comment = generate_doc_comment(&def.doc_comment),
         name = def.name,)?;
    
    file.increase_indent();

    for (idx, field) in def.variants.iter().enumerate() {
        let first = idx == 0;
        generate_variant_def(field, first, file)?;
    }

    file.empty_lines(2)?;

    Ok(())
}


/// Add parenthesis if necessary.
///
/// Wraps `s` in parentheses if it contains a space.
fn to_atom(s: String) -> String {
    if s.contains(' ') {
        format!("({})", s)
    } else {
        s
    }
}

/// Generate elm code for a variant definition.
fn generate_variant_def(variant: &ast::VariantDef, first : bool, file :&mut IndentWriter) -> Result<(), LibError> {
    let delimiter = if first { "= " } else { "| " };
    match variant.variant_type {
        ast::VariantType::Simple => {
            write!(file.start_line()?, "{delimiter}{name}",
                delimiter = delimiter, 
                name = variant.name,
            )?;
        },
        ast::VariantType::Tuple(ref fields) => {
            write!(file.start_line()?, "{delimiter}{name} {fields}",
                delimiter = delimiter, 
                name = variant.name,
                fields = fields
                .elements()
                .iter()
                .map(generate_type_ident)
                .map(to_atom)
                .join(" ")
            )?;
        }
        ast::VariantType::Struct(ref fields) => {
            write!(file.start_line()?, "{delimiter}{name}",
                delimiter = delimiter, 
                name = variant.name,
            )?;
            generate_struct_fields(fields, file)?;
        }
        ast::VariantType::Newtype(ref ty) => {
            write!(file.start_line()?, "{delimiter}{name} {field}",
                delimiter = delimiter, 
                name = variant.name,
                field = to_atom(generate_type_ident(ty))
            )?;
        }
    }

    Ok(())
}

/// Generate elm code for a type identifier.
fn generate_type_ident(type_ident: &ast::TypeIdent) -> String {
    match type_ident {
        ast::TypeIdent::BuiltIn(atom) => generate_atom(atom),
        ast::TypeIdent::List(inner) => format!("List {}", to_atom(generate_type_ident(inner))),
        ast::TypeIdent::Option(inner) => format!("Maybe {}", to_atom(generate_type_ident(inner))),
        ast::TypeIdent::Result(ok, err) => format!(
            "Result {} {}",
            to_atom(generate_type_ident(err)),
            to_atom(generate_type_ident(ok)),
        ),
        ast::TypeIdent::Map(key, value) => format!(
            "Dict {} {}",
            to_atom(generate_type_ident(key)),
            to_atom(generate_type_ident(value)),
        ),
        ast::TypeIdent::Tuple(tdef) => generate_tuple_def(tdef),
        ast::TypeIdent::UserDefined(ident) => ident.to_owned(),
    }
}

/// Generate elm code for a tuple definition.
fn generate_tuple_def(tdef: &ast::TupleDef) -> String {
    format!(
        "({})",
        tdef.elements().iter().map(generate_type_ident).join(", ")
    )
}

/// Generate elm code for an atomic type.
fn generate_atom(atom: &ast::AtomType) -> String {
    match atom {
        ast::AtomType::Empty => "()",
        ast::AtomType::Str => "String",
        ast::AtomType::I32 => "Int",
        ast::AtomType::U32 => "Int",
        ast::AtomType::U8 => "Int",
        ast::AtomType::F64 => "Float",
        ast::AtomType::Bool => "Bool",
        ast::AtomType::DateTime => "Time.Posix",
        ast::AtomType::Date => "Date.Date",
        ast::AtomType::Uuid => "String",
        ast::AtomType::Bytes => "String",
    }
    .to_owned()
}

mod decoder_generation {
    use super::{to_atom, to_camel_case};
    use crate::ast;

    use itertools::Itertools; // directly call join(.) on iterators

    /// Generate elm code for decoders for a spec.
    pub fn generate_type_decoders(spec: &ast::Spec) -> String {
        spec.iter()
            .filter_map(|spec_item| match spec_item {
                ast::SpecItem::StructDef(sdef) => Some(generate_struct_decoder(sdef)),
                ast::SpecItem::EnumDef(edef) => Some(generate_enum_decoder(edef)),
                ast::SpecItem::ServiceDef(_) => None,
            })
            .join("\n\n\n")
    }

    /// Generate elm code for helper functions for enum decoders.
    pub fn generate_enum_helpers(edef: &ast::EnumDef) -> String {
        format!(
            "{fname} : String -> Maybe {type_name}\n\
            {fname} s = case s of \n\
            {variant_decoders}\n\
            {indent}_ -> Nothing\n",
            fname = enum_string_decoder_name(&edef.name),
            type_name = edef.name,
            variant_decoders = edef
                .simple_variants()
                .map(|variant| format!("  \"{name}\" -> Just {name}", name = variant.name))
                .join("\n\n"),
            indent = "  ",
        )
    }

    /// Generate elm code for decoder for a struct.
    fn generate_struct_decoder(sdef: &ast::StructDef) -> String {
        format!(
            "{dec_name} : D.Decoder {name} \n\
            {dec_name} =\n   D.succeed {name}\n        {field_decoders}",
            dec_name = decoder_name(&sdef.name),
            name = sdef.name,
            field_decoders = sdef
                .fields
                .iter()
                .map(generate_field_decoder)
                .join("\n        ")
        )
    }

    /// Generate elm code for decoder for an enum.
    fn generate_enum_decoder(edef: &ast::EnumDef) -> String {
        let optional_string_decoder = if edef.simple_variants().count() > 0 {
            format!(
                "unwrapDecoder (D.map {string_enum_parser} D.string){opt_comma}",
                string_enum_parser = enum_string_decoder_name(&edef.name),
                opt_comma = if edef.complex_variants().count() > 0 {
                    "\n        ,"
                } else {
                    ""
                }
            )
        } else {
            "".to_owned()
        };

        let mut fields = edef.complex_variants().map(|variant| {
            format!(
                "D.field \"{field_name}\" {type_dec}",
                field_name = variant.name,
                type_dec = to_atom(generate_variant_decoder(variant)),
            )
        });

        format!(
            "{dec_name} : D.Decoder {name}\n{dec_name} =\n    D.oneOf\n        [{optional_string_decoder} {fields}\n        ]",
            dec_name = decoder_name(&edef.name),
            name = edef.name,
            optional_string_decoder = optional_string_decoder,
            fields = fields.join("\n        ,"),
        )
    }

    /// Generate elm code for decoder for a field.
    fn generate_field_decoder(field: &ast::FieldNode) -> String {
        format!(
            "|> required \"{name}\" {decoder}",
            name = field.pair.name,
            decoder = to_atom(generate_type_decoder(&field.pair.type_ident)),
        )
    }

    /// Generate elm code for decoder for an enum variant.
    fn generate_variant_decoder(variant: &ast::VariantDef) -> String {
        match variant.variant_type {
            ast::VariantType::Simple => {
                unreachable!("cannot build enum decoder for simple variant")
            }
            ast::VariantType::Tuple(ref components) => format!(
                "D.succeed {name} {components}",
                name = variant.name,
                components = generate_components_by_index_pipeline(components)
            ),
            ast::VariantType::Struct(ref fields) => format!(
                "D.succeed {name} {field_decoders}",
                name = variant.name,
                field_decoders = fields.iter().map(generate_field_decoder).join(" "),
            ),
            ast::VariantType::Newtype(ref ty) => format!(
                "D.map {name} {ty}",
                name = variant.name,
                ty = to_atom(generate_type_decoder(ty)),
            ),
        }
    }

    /// Generate elm code for a decoder for a type.
    fn generate_type_decoder(type_ident: &ast::TypeIdent) -> String {
        match type_ident {
            ast::TypeIdent::BuiltIn(atom) => generate_atom_decoder(atom),
            ast::TypeIdent::List(inner) => {
                format!("D.list {}", to_atom(generate_type_decoder(inner)))
            }
            ast::TypeIdent::Option(inner) => {
                format!("D.maybe {}", to_atom(generate_type_decoder(inner)))
            }
            ast::TypeIdent::Result(_ok, _err) => todo!(),
            ast::TypeIdent::Map(key, value) => {
                assert_eq!(
                    generate_type_decoder(key),
                    "D.string",
                    "elm only supports dict keys"
                );
                format!("D.dict {}", to_atom(generate_type_decoder(value)))
            }
            ast::TypeIdent::Tuple(tdef) => generate_tuple_decoder(tdef),
            ast::TypeIdent::UserDefined(ident) => decoder_name(ident),
        }
    }

    /// Generate elm code for a decoder for a tuple.
    fn generate_tuple_decoder(tdef: &ast::TupleDef) -> String {
        let len = tdef.elements().len();
        let parts: Vec<String> = (0..len).map(|i| format!("x{}", i)).collect();

        format!(
            "D.succeed (\\{tuple_from} -> ({tuple_to})) {field_decoders}",
            tuple_from = parts.iter().join(" "),
            tuple_to = parts.iter().join(", "),
            field_decoders = generate_components_by_index_pipeline(tdef),
        )
    }

    /// Generate elm code for a pipeline that decodes tuple fields by index.
    fn generate_components_by_index_pipeline(tuple: &ast::TupleDef) -> String {
        tuple
            .elements()
            .iter()
            .enumerate()
            .map(|(index, element)| {
                let decoder = to_atom(generate_type_decoder(&element));
                format!("|> requiredIdx {} {}", index, decoder)
            })
            .join(" ")
    }

    /// Generate elm code for a decoder for an atomic type.
    fn generate_atom_decoder(atom: &ast::AtomType) -> String {
        match atom {
            ast::AtomType::Empty => "(D.succeed ())",
            ast::AtomType::Str => "D.string",
            ast::AtomType::I32 => "D.int",
            ast::AtomType::U32 => "D.int",
            ast::AtomType::U8 => "D.int",
            ast::AtomType::F64 => "D.float",
            ast::AtomType::Bool => "D.bool",
            ast::AtomType::DateTime => "Iso8601.decoder",
            ast::AtomType::Date => "dateDecoder",
            ast::AtomType::Uuid => todo!(),
            ast::AtomType::Bytes => todo!(),
        }
        .to_string()
    }

    /// Construct decoder function name.
    fn decoder_name(ident: &str) -> String {
        to_camel_case(&format!("{}Decoder", ident))
    }

    /// Construct function name for an enum decoder.
    fn enum_string_decoder_name(ident: &str) -> String {
        to_camel_case(&format!("parseEnum{}FromString", ident))
    }
}

mod encoder_generation {
    use super::{field_name, to_atom, to_camel_case};
    use crate::ast;

    use itertools::Itertools;

    /// Generate elm code for encoder functions for `spec`.
    pub fn generate_type_encoders(spec: &ast::Spec) -> String {
        spec.iter()
            .filter_map(|spec_item| match spec_item {
                ast::SpecItem::StructDef(sdef) => Some(generate_struct_encoder(sdef)),
                ast::SpecItem::EnumDef(edef) => Some(generate_enum_encoder(edef)),
                ast::SpecItem::ServiceDef(_) => None,
            })
            .join("\n\n\n")
    }

    /// Generate elm code for a struct encoder.
    fn generate_struct_encoder(sdef: &ast::StructDef) -> String {
        format!(
            "{encoder_name} : {type_name} -> E.Value\n{encoder_name} obj =\n    E.object\n        [ {fields}\n        ]",
            encoder_name = encoder_name(&sdef.name),
            type_name = sdef.name,
            fields = sdef.fields.iter().map(generate_field_encoder).join("\n        , "),
        )
    }

    /// Generate elm code for an enum encoder.
    fn generate_enum_encoder(edef: &ast::EnumDef) -> String {
        format!(
            "{encoder_name} : {type_name} -> E.Value\n{encoder_name} v =\n    case v of\n        {variants}",
            encoder_name = encoder_name(&edef.name),
            type_name = edef.name,
            variants = edef
                .variants
                .iter()
                .map(generate_variant_encoder_branch)
                .join("\n        "),
        )
    }

    /// Generate elm code for a field encoder.
    fn generate_field_encoder(field: &ast::FieldNode) -> String {
        format!(
            "(\"{name}\", {value_encoder} obj.{field_name})",
            name = field.pair.name,
            field_name = field_name(&field.pair.name),
            value_encoder = to_atom(generate_type_encoder(&field.pair.type_ident))
        )
    }

    /// Generate elm code for encoding code for variant of enum.
    fn generate_variant_encoder_branch(variant: &ast::VariantDef) -> String {
        match variant.variant_type {
            ast::VariantType::Simple => format!("{name} -> E.string \"{name}\"", name = variant.name),
            ast::VariantType::Tuple(ref tdef) => format!(
                "{name} {field_names} -> E.object [ (\"{name}\", E.list identity [{field_encoders}]) ]",
                name = variant.name,
                field_names = (0..tdef.elements().len())
                    .map(|i| format!("x{}", i))
                    .join(" "),
                field_encoders = tdef
                    .elements()
                    .iter()
                    .enumerate()
                    .map(|(idx, component)| format!("{} x{}", generate_type_encoder(component), idx))
                    .join(", "),
            ),
            ast::VariantType::Struct(ref fields) => format!(
                "{name} obj -> E.object [ (\"{name}\", E.object [{fields}]) ]",
                name = variant.name,
                fields = fields.iter().map(generate_field_encoder).join(", "),
            ),
            ast::VariantType::Newtype(ref ty) => format!(
                "{name} obj -> E.object [ (\"{name}\", {enc} obj) ]",
                name = variant.name,
                enc = generate_type_encoder(ty),
            ),
        }
    }

    /// Generate elm code for a type encoder.
    fn generate_type_encoder(type_ident: &ast::TypeIdent) -> String {
        match type_ident {
            ast::TypeIdent::BuiltIn(atom) => generate_atom_encoder(atom),
            ast::TypeIdent::List(inner) => {
                format!("E.list {}", to_atom(generate_type_encoder(inner)))
            }
            ast::TypeIdent::Option(inner) => {
                format!("encMaybe {}", to_atom(generate_type_encoder(inner)))
            }
            ast::TypeIdent::Result(_ok, _err) => todo!(),
            ast::TypeIdent::Map(key, value) => {
                assert_eq!(
                    generate_type_encoder(key),
                    "E.string",
                    "can only encode string keys in maps"
                );
                format!("E.dict identity {}", to_atom(generate_type_encoder(value)))
            }
            ast::TypeIdent::Tuple(tdef) => generate_tuple_encoder(tdef),
            ast::TypeIdent::UserDefined(ident) => encoder_name(ident),
        }
    }

    /// Generate elm code for an atomic type encoder.
    fn generate_atom_encoder(atom: &ast::AtomType) -> String {
        match atom {
            ast::AtomType::Empty => "(_ -> E.null)",
            ast::AtomType::Str => "E.string",
            ast::AtomType::I32 => "E.int",
            ast::AtomType::U32 => "E.int",
            ast::AtomType::U8 => "E.int",
            ast::AtomType::F64 => "E.float",
            ast::AtomType::Bool => "E.bool",
            ast::AtomType::DateTime => "Iso8601.encode",
            ast::AtomType::Date => "encDate",
            ast::AtomType::Uuid => todo!(),
            ast::AtomType::Bytes => todo!(),
        }
        .to_owned()
    }

    /// Generate elm code for a tuple encoder.
    fn generate_tuple_encoder(tdef: &ast::TupleDef) -> String {
        format!(
            "\\({field_names}) -> E.list identity [ {encode_values} ]",
            field_names = (0..tdef.elements().len())
                .map(|i| format!("x{}", i))
                .join(", "),
            encode_values = tdef
                .elements()
                .iter()
                .enumerate()
                .map(|(idx, component)| format!("{} x{}", generate_type_encoder(component), idx))
                .join(", "),
        )
    }

    /// Construct name of encoder function for specific `ident`.
    fn encoder_name(ident: &str) -> String {
        to_camel_case(&format!("encode{}", ident))
    }
}

/// Construct name for a field.
fn field_name(ident: &str) -> String {
    to_camel_case(ident)
}

fn generate_rest_api_client_helpers(spec: &ast::Spec) -> String {
    spec.iter()
        .filter_map(|spec_item| match spec_item {
            ast::SpecItem::StructDef(_) | ast::SpecItem::ServiceDef(_) => None,
            ast::SpecItem::EnumDef(edef) => Some(decoder_generation::generate_enum_helpers(edef)),
        })
        .join("")
}

fn generate_rest_api_clients(spec: &ast::Spec) -> String {
    generate_rest_api_client_helpers(spec);
    spec.iter()
        .filter_map(|spec_item| match spec_item {
            // No helpers for structs.
            ast::SpecItem::StructDef(_) | ast::SpecItem::EnumDef(_) => None,
            ast::SpecItem::ServiceDef(service) => Some(generate_rest_api_client(service)),
        })
        .join("")
}

fn generate_rest_api_client(spec: &ast::ServiceDef) -> String {
    todo!()
}

pub struct Generator {
    _artifact: Artifact,
}

impl Generator {
    pub fn new(artifact: Artifact) -> Result<Self, LibError> {
        match artifact {
            Artifact::TypesOnly | Artifact::ClientEndpoints => Ok(Self { _artifact: artifact }),
            Artifact::ServerEndpoints => Err(LibError::UnsupportedArtifact {
                artifact,
                backend: BACKEND_NAME,
            }),
        }
    }

    pub fn generate_user_defined_types(spec :&Spec, outdir: &Path) -> Result<(), LibError> {
        // TODO: populate mem filesystem or temp folder first, then make everything visible at once
        // to avoid partial write out on error
        let mut file = IndentWriter::for_file(outdir, "Data.elm")?;

        // TODO: make module path prefix configurable
        write!(file.handle(), "module Api.Data exposing (..)")?;
        file.empty_lines(2)?;
        
        for spec_item in spec.iter() {
            match spec_item {
                ast::SpecItem::StructDef(sdef) => generate_struct_def(sdef, &mut file)?,
                ast::SpecItem::EnumDef(edef) => generate_enum_def(edef, &mut file)?,
                ast::SpecItem::ServiceDef(_) => {},
            };
        }

        Ok(())
    }

    // pub fn generate_spec(&self, spec: &Spec) -> String {
    //     let generate_client_side_services = self.artifact == Artifact::ClientEndpoints
    //         && spec
    //             .iter()
    //             .find(|item| item.service_def().is_some())
    //             .is_some();

    //     let defs = generate_def(spec);

    //     let mut outfile = vec![
    //         include_str!("elm/module_header.elm"),
    //         include_str!("elm/preamble_types.elm"),
    //         if generate_client_side_services {
    //             include_str!("elm/preamble_services.elm")
    //         } else {
    //             ""
    //         },
    //         &defs,
    //         include_str!("elm/utils_types.elm"),
    //     ];

    //     if generate_client_side_services {
    //         let decoders = decoder_generation::generate_type_decoders(spec);
    //         let encoders = encoder_generation::generate_type_encoders(spec);
    //         let clients = ""; //generate_rest_api_clients(spec);
    //         let client_side_code: Vec<&str> = vec![&decoders, &encoders, &clients];
    //         outfile.extend(client_side_code);
    //         outfile.join("\n")
    //     } else {
    //         outfile.join("\n")
    //     }
    // }

    pub fn validate_output_dir(path: &Path) -> Result<(), LibError> {
        if !path.is_dir() {
            return Err(LibError::OutputMustBeFolder {
                backend: BACKEND_NAME,
            });
        }

        let is_empty = path.read_dir().map_err(LibError::IoError)?.next().is_none();

        if !is_empty {
            return Err(LibError::OutputFolderNotEmpty {
                backend: BACKEND_NAME,
            });
        }

        Ok(())
    }
}

impl crate::CodeGenerator for Generator {
    fn generate(&self, spec: &Spec, output: &Path) -> Result<(), LibError> {
        Self::validate_output_dir(&output)?;

        Self::generate_user_defined_types(&spec, &output)?;
        //let generated_code = self.generate_spec(spec);

        //let mut outdir = PathBuf::from(&output);

        Ok(())
    }
}
