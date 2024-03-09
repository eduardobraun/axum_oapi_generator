use std::collections::BTreeMap;

use heck::ToSnakeCase;
use openapiv3::{OpenAPI, Operation, Parameter, PathItem, ReferenceOr};
use quote::{format_ident, quote};
use syn::{Expr, FnArg, Item, ReturnType, Type};

struct OapiState {
    _type_cache: BTreeMap<String, syn::Type>,
    _objects: BTreeMap<String, Item>,
    methods: BTreeMap<String, Item>,
}

impl OapiState {
    fn new() -> Self {
        OapiState {
            _type_cache: BTreeMap::default(),
            _objects: BTreeMap::default(),
            methods: BTreeMap::default(),
        }
    }

    fn _add_object(&mut self, name: impl AsRef<str>, object: Item) -> anyhow::Result<()> {
        if self._objects.contains_key(name.as_ref()) {
            return Err(anyhow::anyhow!("object with this name alredy exist"));
        }
        self._objects.insert(name.as_ref().to_owned(), object);
        Ok(())
    }

    fn add_method(&mut self, name: impl AsRef<str>, method: Item) -> anyhow::Result<()> {
        if self.methods.contains_key(name.as_ref()) {
            return Err(anyhow::anyhow!("method with this name alredy exist"));
        }
        self.methods.insert(name.as_ref().to_owned(), method);
        Ok(())
    }
}

pub(crate) fn generate(oapi_definition: &OpenAPI) -> anyhow::Result<BTreeMap<String, String>> {
    let mut files: BTreeMap<String, String> = BTreeMap::default();
    let mut state = OapiState::new();

    for (path_name, path_or_ref) in oapi_definition.paths.paths.iter() {
        match path_or_ref {
            openapiv3::ReferenceOr::Reference { reference } => {
                let _ = reference;
                unimplemented!()
            }
            openapiv3::ReferenceOr::Item(path_item) => {
                let path_arguments =
                    generate_path_args(&oapi_definition, &mut state, &path_item.parameters);
                for (method, operation) in MethodsIterator::new(&path_item) {
                    generate_operation(
                        &oapi_definition,
                        &mut state,
                        &path_arguments,
                        method,
                        path_name,
                        operation,
                    );
                }
            }
        }
    }

    let file = syn::File {
        attrs: vec![],
        items: state.methods.values().cloned().collect(),
        shebang: None,
    };
    let file_content = prettyplease::unparse(&file).replace("type newline = ();", "");
    files.insert("somefile.rs".to_string(), file_content);

    Ok(files)
}

fn get_parameter_type(parameter: &Parameter) -> Type {
    let required = parameter.parameter_data_ref().required;
    let r#type = match &parameter.parameter_data_ref().format {
        openapiv3::ParameterSchemaOrContent::Schema(schema_or_ref) => match schema_or_ref {
            ReferenceOr::Reference { reference } => {
                let _ = reference;
                unimplemented!()
            }
            ReferenceOr::Item(schema) => match &schema.schema_kind {
                openapiv3::SchemaKind::Type(schema_type) => match schema_type {
                    openapiv3::Type::String(_) => "String",
                    openapiv3::Type::Number(_) => "f64",
                    openapiv3::Type::Integer(_) => "i64",
                    openapiv3::Type::Object(_) => unimplemented!(),
                    openapiv3::Type::Array(_) => unimplemented!(),
                    openapiv3::Type::Boolean(_) => "bool",
                },
                _ => unimplemented!(),
            },
        },
        openapiv3::ParameterSchemaOrContent::Content(_) => unimplemented!(),
    };
    let r#type = if required {
        r#type.to_owned()
    } else {
        format!("Option<{}>", r#type)
    };
    syn::parse_str::<Type>(&r#type).unwrap()
}

fn generate_path_args(
    definition: &OpenAPI,
    _state: &mut OapiState,
    parameters: &[ReferenceOr<Parameter>],
) -> Option<FnArg> {
    let parameters: Vec<_> = parameters
        .iter()
        .map(|param_or_ref| match param_or_ref {
            ReferenceOr::Reference { reference } => {
                let s: Vec<_> = reference.split("/").collect();
                let param = definition
                    .components
                    .as_ref()
                    .unwrap()
                    .parameters
                    .get(s.last().unwrap().to_owned())
                    .unwrap()
                    .as_item()
                    .unwrap();

                param
            }
            ReferenceOr::Item(param) => param,
        })
        .collect();

    let path_names: Vec<_> = parameters
        .iter()
        .map(|param| format_ident!("{}", param.parameter_data_ref().name.to_snake_case()))
        .collect();
    let path_types: Vec<_> = parameters
        .iter()
        .map(|param| get_parameter_type(param))
        .collect();
    if !path_names.is_empty() {
        let p = quote!(Path(( #( #path_names , )* )) : Path<( #( #path_types , )* )>);
        Some(syn::parse2(p).unwrap())
    } else {
        None
    }
}

fn generate_operation_docs(
    method: impl AsRef<str>,
    path: impl AsRef<str>,
    operation: &Operation,
) -> Vec<String> {
    let mut docs = vec![];
    docs.push(Some(format!(
        " [{}] {}",
        method.as_ref().to_uppercase(),
        path.as_ref()
    )));
    docs.push(
        operation
            .summary
            .as_ref()
            .map(|summary| format!(" {}", summary)),
    );
    if let Some(desc) = operation.description.as_ref().map(|description| {
        textwrap::wrap(description, 80)
            .iter_mut()
            .map(|line| Some(format!(" {}", line)))
            .collect::<Vec<Option<String>>>()
    }) {
        docs.extend(desc);
    };
    docs.into_iter().flatten().collect()
}

fn generate_operation_args(
    _definition: &OpenAPI,
    _state: &mut OapiState,
    path_arguments: &Option<FnArg>,
    operation: &Operation,
) -> Vec<FnArg> {
    // Add ApiState to every method
    let mut fn_args: Vec<FnArg> =
        vec![syn::parse_str::<FnArg>("State(state): State<ApiState>").expect("this should parse")];

    if let Some(arg) = path_arguments {
        fn_args.push(arg.to_owned());
    }

    let mut arg_idents: Vec<Expr> = vec![];
    let mut arg_types: Vec<Type> = vec![];
    for parameter_or_ref in operation.parameters.iter() {
        match parameter_or_ref {
            ReferenceOr::Reference { reference } => {
                let _ = reference;
                unimplemented!()
            }
            ReferenceOr::Item(parameter) => {
                let arg_name = format!("{}", parameter.parameter_data_ref().name.to_snake_case());
                let ident = syn::parse_str::<Expr>(&arg_name).unwrap();
                let ty = get_parameter_type(parameter);
                arg_idents.push(ident);
                arg_types.push(ty);
            }
        };
    }

    for (ident, ty) in arg_idents.iter().zip(arg_types.iter()) {
        let arg = quote!(Query( #ident ) : Query<#ty>);
        fn_args.push(syn::parse2::<FnArg>(arg).unwrap());
    }

    let request_arg = operation
        .request_body
        .as_ref()
        .map(|request_or_ref| match request_or_ref {
            ReferenceOr::Reference { reference } => {
                let _ = reference;
                unimplemented!();
            }
            ReferenceOr::Item(request_body) => {
                request_body
                    .content
                    .first()
                    .map(|(media_type, _value)| match media_type.as_str() {
                        "application/json" => {
                            let request_arg = format!("Json(request): Json<TODO>");
                            syn::parse_str::<FnArg>(&request_arg).unwrap()
                        }
                        "application/x-www-form-urlencoded" => {
                            let request_arg = format!("Form(request): Form<TODO>");
                            syn::parse_str::<FnArg>(&request_arg).unwrap()
                        }
                        _ => unimplemented!(),
                    })
            }
        })
        .flatten();

    if let Some(arg) = request_arg {
        fn_args.push(arg);
    }

    fn_args
}

fn generate_operation_response() -> Option<ReturnType> {
    Some(syn::parse_str("-> Result<TODO, TODO>").unwrap())
}

fn generate_operation(
    definition: &OpenAPI,
    state: &mut OapiState,
    path_arguments: &Option<FnArg>,
    method: impl AsRef<str>,
    path: impl AsRef<str>,
    operation: &Operation,
) {
    let operation_docs = generate_operation_docs(method, path, operation);
    let operation_name = operation.operation_id.as_ref().unwrap().to_snake_case();
    let operation_ident = format_ident!("{}", operation_name);
    let operation_args = generate_operation_args(definition, state, path_arguments, operation);
    let operation_response = generate_operation_response();

    let tokens = quote! {
        #( #[doc = #operation_docs] )*
        pub async fn #operation_ident ( #( #operation_args ,)* ) #operation_response {
            todo!();
        }
    };
    state
        .add_method(operation_name, syn::parse2::<Item>(tokens).unwrap())
        .expect("operation_name should be unique");
}

struct MethodsIterator<'a> {
    path_item: &'a PathItem,
    step: u8,
}

impl<'a> MethodsIterator<'a> {
    fn new(path_item: &'a PathItem) -> Self {
        MethodsIterator { path_item, step: 0 }
    }
}

impl<'a> Iterator for MethodsIterator<'a> {
    type Item = (String, &'a Operation);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.step > 6 {
                return None;
            }
            let m = match self.step {
                0 => self
                    .path_item
                    .options
                    .as_ref()
                    .map(|op| ("options".to_owned(), op)),
                1 => self
                    .path_item
                    .head
                    .as_ref()
                    .map(|op| ("head".to_owned(), op)),
                2 => self.path_item.get.as_ref().map(|op| ("get".to_owned(), op)),
                3 => self
                    .path_item
                    .post
                    .as_ref()
                    .map(|op| ("post".to_owned(), op)),
                4 => self
                    .path_item
                    .delete
                    .as_ref()
                    .map(|op| ("delete".to_owned(), op)),
                5 => self
                    .path_item
                    .patch
                    .as_ref()
                    .map(|op| ("patch".to_owned(), op)),
                6 => self
                    .path_item
                    .trace
                    .as_ref()
                    .map(|op| ("trace".to_owned(), op)),
                _ => None,
            };
            self.step = self.step.saturating_add(1);
            if m.is_some() {
                return m;
            }
        }
    }
}
