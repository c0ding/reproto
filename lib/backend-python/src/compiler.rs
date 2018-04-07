//! Python Compiler

use backend::{PackageProcessor, PackageUtils};
use codegen::{EndpointExtra, ServiceAdded, ServiceCodegen};
use core::errors::*;
use core::{self, ForEachLoc, Handle, Loc, RelativePathBuf};
use flavored::{PythonFlavor, PythonName, RpEnumBody, RpField, RpInterfaceBody, RpPackage,
               RpServiceBody, RpTupleBody, RpTypeBody};
use genco::python::{imported, Python};
use genco::{Element, Quoted, Tokens};
use naming::{self, Naming};
use std::collections::BTreeMap;
use std::iter;
use std::rc::Rc;
use trans::{self, Translated};
use {FileSpec, Options, PythonPackageUtils, EXT, INIT_PY};

pub struct Compiler<'el> {
    pub env: &'el Translated<PythonFlavor>,
    variant_field: &'el Loc<RpField>,
    to_lower_snake: naming::ToLowerSnake,
    dict: Element<'static, Python<'static>>,
    enum_enum: Python<'static>,
    service_generators: Vec<Box<ServiceCodegen>>,
    handle: &'el Handle,
}

impl<'el> Compiler<'el> {
    pub fn new(
        env: &'el Translated<PythonFlavor>,
        variant_field: &'el Loc<RpField>,
        options: Options,
        handle: &'el Handle,
    ) -> Compiler<'el> {
        Compiler {
            env,
            variant_field,
            to_lower_snake: naming::to_lower_snake(),
            dict: "dict".into(),
            enum_enum: imported("enum").name("Enum"),
            service_generators: options.service_generators,
            handle,
        }
    }

    /// Compile the given backend.
    pub fn compile(&self) -> Result<()> {
        self.write_files(self.populate_files()?)
    }

    /// Build a function that raises an exception if the given value `toks` is None.
    fn raise_if_none(
        &self,
        toks: Tokens<'el, Python<'el>>,
        field: &RpField,
    ) -> Tokens<'el, Python<'el>> {
        let mut raise_if_none = Tokens::new();
        let required_error = format!("{}: is a required field", field.name()).quoted();

        raise_if_none.push(toks!["if ", toks, " is None:"]);
        raise_if_none.nested(toks!["raise Exception(", required_error, ")"]);

        raise_if_none
    }

    fn encode_method<I>(
        &self,
        fields: I,
        builder: Tokens<'el, Python<'el>>,
        extra: Option<Tokens<'el, Python<'el>>>,
    ) -> Result<Tokens<'el, Python<'el>>>
    where
        I: IntoIterator<Item = &'el Loc<RpField>>,
    {
        let mut encode_body = Tokens::new();

        encode_body.push(toks!["data = ", builder.clone(), "()"]);

        if let Some(extra) = extra {
            encode_body.push(extra);
        }

        for field in fields {
            let var_string = field.name().quoted();
            let field_toks = toks!["self.", field.safe_ident()];

            let value_toks = field.ty.encode(field_toks.clone());

            if field.is_optional() {
                let mut check_if_none = Tokens::new();

                check_if_none.push(toks!["if ", field_toks, " is not None:"]);

                let toks = toks!["data[", var_string, "] = ", value_toks];

                check_if_none.nested(toks);

                encode_body.push(check_if_none);
            } else {
                encode_body.push(self.raise_if_none(field_toks, field));

                let toks = toks!["data[", var_string, "] = ", value_toks];

                encode_body.push(toks);
            }
        }

        encode_body.push(toks!["return data"]);

        let mut encode = Tokens::new();
        encode.push("def encode(self):");
        encode.nested(encode_body.join_line_spacing());
        Ok(encode)
    }

    fn encode_tuple_method<I>(&self, fields: I) -> Result<Tokens<'el, Python<'el>>>
    where
        I: IntoIterator<Item = &'el Loc<RpField>>,
    {
        let mut values = Tokens::new();
        let mut encode_body = Tokens::new();

        for field in fields.into_iter() {
            let toks = toks!["self.", field.safe_ident()];
            encode_body.push(self.raise_if_none(toks.clone(), field));
            values.append(field.ty.encode(toks));
        }

        encode_body.push(toks!["return (", values.join(", "), ")"]);

        let mut encode = Tokens::new();
        encode.push("def encode(self):");
        encode.nested(encode_body.join_line_spacing());
        Ok(encode)
    }

    fn repr_method<I>(&self, name: &'el PythonName, fields: I) -> Tokens<'el, Python<'el>>
    where
        I: IntoIterator<Item = &'el Loc<RpField>>,
    {
        let mut args = Vec::new();
        let mut vars = Tokens::new();

        for field in fields {
            args.push(format!("{}:{{!r}}", field.ident.as_str()));
            vars.append(toks!["self.", field.safe_ident()]);
        }

        let format = if !args.is_empty() {
            format!("<{} {}>", name, args.join(", "))
        } else {
            format!("<{}>", name)
        };

        let mut repr = Tokens::new();
        repr.push("def __repr__(self):");
        repr.nested(toks![
            "return ",
            format.quoted(),
            ".format(",
            vars.join(", "),
            ")",
        ]);
        repr
    }

    fn optional_check(
        &self,
        var: Tokens<'el, Python<'el>>,
        index: Tokens<'el, Python<'el>>,
        toks: Tokens<'el, Python<'el>>,
    ) -> Tokens<'el, Python<'el>> {
        let mut check = Tokens::new();

        let mut none_check = Tokens::new();
        none_check.push(toks![var.clone(), " = data[", index.clone(), "]"]);

        let mut none_check_if = Tokens::new();

        let assign_var = toks![var.clone(), " = ", toks];

        none_check_if.push(toks!["if ", var.clone(), " is not None:"]);
        none_check_if.nested(assign_var);

        none_check.push(none_check_if);

        check.push(toks!["if ", index.clone(), " in data:"]);
        check.nested(none_check.join_line_spacing());

        check.push(toks!["else:"]);
        check.nested(toks![var.clone(), " = None"]);

        check.into()
    }

    fn decode_method<F, I>(
        &self,
        name: &'el PythonName,
        fields: I,
        variable_fn: F,
    ) -> Result<Tokens<'el, Python<'el>>>
    where
        F: Fn(usize, &'el RpField) -> Tokens<'el, Python<'el>>,
        I: IntoIterator<Item = &'el Loc<RpField>>,
    {
        let mut body = Tokens::new();
        let mut args = Tokens::new();

        for (i, field) in fields.into_iter().enumerate() {
            let var_name = Rc::new(format!("f_{}", field.ident));
            let var = variable_fn(i, field);

            let toks = if field.is_optional() {
                let var_name = toks!(var_name.clone());
                let var_toks = field.ty.decode(var_name.clone());
                self.optional_check(var_name.clone(), var, var_toks)
            } else {
                let data = toks!["data[", var.clone(), "]"];
                let var_toks = field.ty.decode(data);
                toks![var_name.clone(), " = ", var_toks]
            };

            body.push(toks);
            args.append(toks!(var_name));
        }

        let args = args.join(", ");
        body.push(toks!["return ", name, "(", args, ")"]);

        let mut decode = Tokens::new();
        decode.push("@staticmethod");
        decode.push("def decode(data):");
        decode.nested(body.join_line_spacing());

        Ok(decode)
    }

    fn build_constructor<I>(&self, fields: I) -> Tokens<'el, Python<'el>>
    where
        I: IntoIterator<Item = &'el Loc<RpField>>,
    {
        let mut args = Tokens::new();
        let mut assign = Tokens::new();

        args.append("self");

        for field in fields {
            args.append(field.safe_ident());

            assign.push(toks![
                "self.",
                field.safe_ident(),
                " = ",
                field.safe_ident(),
            ]);
        }

        let mut constructor = Tokens::new();
        constructor.push(toks!["def __init__(", args.join(", "), "):"]);

        if assign.is_empty() {
            constructor.nested("pass");
        } else {
            constructor.nested(assign);
        }

        constructor
    }

    fn build_getters<I>(&self, fields: I) -> Result<Vec<Tokens<'el, Python<'el>>>>
    where
        I: IntoIterator<Item = &'el Loc<RpField>>,
    {
        let mut result = Vec::new();

        for field in fields {
            let name = Rc::new(self.to_lower_snake.convert(field.ident.as_str()));
            let mut body = Tokens::new();
            body.push(toks!("def get_", name, "(self):"));

            body.nested({
                let mut t = Tokens::new();

                if !field.comment.is_empty() {
                    t.push("\"\"\"");

                    for c in &field.comment {
                        t.push(Element::from(c.clone()));
                    }

                    t.push("\"\"\"");
                }

                t.push(toks!["return self.", field.safe_ident()]);
                t
            });

            result.push(body);
        }

        Ok(result)
    }

    pub fn enum_variants(&self, body: &'el RpEnumBody) -> Result<Tokens<'el, Python<'el>>> {
        let mut args = Tokens::new();

        let variants = body.variants.iter().map(|l| Loc::as_ref(l));

        variants.for_each_loc(|variant| {
            let mut enum_arguments = Tokens::new();

            enum_arguments.append(variant.ident().quoted());
            enum_arguments.append(variant.ordinal().quoted());

            args.append(toks!["(", enum_arguments.join(", "), ")"]);

            Ok(()) as Result<()>
        })?;

        Ok(toks![
            &body.name,
            " = ",
            self.enum_enum.clone(),
            "(",
            body.name.to_string().quoted(),
            ", [",
            args.join(", "),
            "], type=",
            &body.name,
            ")",
        ])
    }

    fn as_class(
        &self,
        name: &'el PythonName,
        body: Tokens<'el, Python<'el>>,
    ) -> Tokens<'el, Python<'el>> {
        let mut class = Tokens::new();
        class.push(toks!("class ", name, ":"));

        if body.is_empty() {
            class.nested("pass");
        } else {
            class.nested(body.join_line_spacing());
        }

        class
    }
}

impl<'el> PackageProcessor<'el, PythonFlavor, PythonName> for Compiler<'el> {
    type Out = FileSpec<'el>;
    type DeclIter = trans::translated::DeclIter<'el, PythonFlavor>;

    fn package_prefix(&self) -> Option<&RpPackage> {
        self.env.package_prefix()
    }

    fn ext(&self) -> &str {
        EXT
    }

    fn decl_iter(&self) -> Self::DeclIter {
        self.env.decl_iter()
    }

    fn handle(&self) -> &'el Handle {
        self.handle
    }

    fn process_tuple(&self, out: &mut Self::Out, body: &'el RpTupleBody) -> Result<()> {
        let mut tuple_body = Tokens::new();

        tuple_body.push(self.build_constructor(&body.fields));

        for getter in self.build_getters(&body.fields)? {
            tuple_body.push(getter);
        }

        tuple_body.push_unless_empty(code!(&body.codes, core::RpContext::Python));

        let decode = self.decode_method(&body.name, &body.fields, |i, _| i.to_string().into())?;
        tuple_body.push(decode);

        let encode = self.encode_tuple_method(&body.fields)?;
        tuple_body.push(encode);

        let repr_method = self.repr_method(&body.name, &body.fields);
        tuple_body.push(repr_method);

        let class = self.as_class(&body.name, tuple_body);

        out.0.push(class);
        Ok(())
    }

    fn process_enum(&self, out: &mut Self::Out, body: &'el RpEnumBody) -> Result<()> {
        let mut class_body = Tokens::new();

        class_body.push(self.build_constructor(iter::once(self.variant_field)));

        for getter in self.build_getters(iter::once(self.variant_field))? {
            class_body.push(getter);
        }

        class_body.push_unless_empty(code!(&body.codes, core::RpContext::Python));

        class_body.push(encode_method(self.variant_field)?);
        class_body.push(decode_method(self.variant_field)?);

        let repr_method = self.repr_method(&body.name, iter::once(self.variant_field));
        class_body.push(repr_method);

        let class = self.as_class(&body.name, class_body);
        out.0.push(class);
        return Ok(());

        fn encode_method<'el>(field: &'el Loc<RpField>) -> Result<Tokens<'el, Python<'el>>> {
            let mut m = Tokens::new();
            m.push("def encode(self):");
            m.nested(toks!["return self.", field.safe_ident()]);
            Ok(m)
        }

        fn decode_method<'el>(field: &'el Loc<RpField>) -> Result<Tokens<'el, Python<'el>>> {
            let mut decode_body = Tokens::new();

            let mut check = Tokens::new();
            check.push(toks!["if value.", field.safe_ident(), " == data:"]);
            check.nested(toks!["return value"]);

            let mut member_loop = Tokens::new();

            member_loop.push("for value in cls.__members__.values():");
            member_loop.nested(check);

            decode_body.push(member_loop);
            decode_body.push(toks![
                "raise Exception(",
                "data does not match enum".quoted(),
                ")",
            ]);

            let mut m = Tokens::new();
            m.push("@classmethod");
            m.push("def decode(cls, data):");
            m.nested(decode_body.join_line_spacing());
            Ok(m)
        }
    }

    fn process_type(&self, out: &mut Self::Out, body: &'el RpTypeBody) -> Result<()> {
        let mut class_body = Tokens::new();

        let constructor = self.build_constructor(&body.fields);
        class_body.push(constructor);

        for getter in self.build_getters(&body.fields)? {
            class_body.push(getter);
        }

        let decode = self.decode_method(&body.name, &body.fields, |_, field| {
            toks!(field.name().quoted())
        })?;

        class_body.push(decode);

        let encode = self.encode_method(&body.fields, self.dict.clone().into(), None)?;

        class_body.push(encode);

        let repr_method = self.repr_method(&body.name, &body.fields);
        class_body.push(repr_method);
        class_body.push_unless_empty(code!(&body.codes, core::RpContext::Python));

        out.0.push(self.as_class(&body.name, class_body));
        Ok(())
    }

    fn process_interface(&self, out: &mut Self::Out, body: &'el RpInterfaceBody) -> Result<()> {
        let mut type_body = Tokens::new();

        match body.sub_type_strategy {
            core::RpSubTypeStrategy::Tagged { ref tag, .. } => {
                let tk = tag.as_str().quoted().into();
                type_body.push(decode(&body, &tk)?);
            }
        }

        type_body.push_unless_empty(code!(&body.codes, core::RpContext::Python));

        out.0.push(self.as_class(&body.name, type_body));

        let values = body.sub_types.iter().map(|l| Loc::as_ref(l));

        values.for_each_loc(|sub_type| {
            let mut sub_type_body = Tokens::new();

            sub_type_body.push(toks!["TYPE = ", sub_type.name().quoted()]);

            let fields: Vec<&Loc<RpField>> =
                body.fields.iter().chain(sub_type.fields.iter()).collect();

            let constructor = self.build_constructor(fields.iter().cloned());
            sub_type_body.push(constructor);

            for getter in self.build_getters(fields.iter().cloned())? {
                sub_type_body.push(getter);
            }

            let decode = self.decode_method(&sub_type.name, fields.iter().cloned(), |_, field| {
                toks!(field.ident.clone().quoted())
            })?;

            sub_type_body.push(decode);

            match body.sub_type_strategy {
                core::RpSubTypeStrategy::Tagged { ref tag, .. } => {
                    let tk: Tokens<'el, Python<'el>> = tag.as_str().quoted().into();

                    let encode = self.encode_method(
                        fields.iter().cloned(),
                        self.dict.clone().into(),
                        Some(toks!["data[", tk, "] = ", sub_type.name().quoted(),]),
                    )?;

                    sub_type_body.push(encode);
                }
            }

            let repr_method = self.repr_method(&sub_type.name, fields.iter().cloned());
            sub_type_body.push(repr_method);
            sub_type_body.push_unless_empty(code!(&sub_type.codes, core::RpContext::Python));

            out.0.push(self.as_class(&sub_type.name, sub_type_body));
            Ok(()) as Result<()>
        })?;

        return Ok(());

        fn decode<'el>(
            body: &'el RpInterfaceBody,
            tag: &Tokens<'el, Python<'el>>,
        ) -> Result<Tokens<'el, Python<'el>>> {
            let mut t = Tokens::new();

            let data = "data";
            let f_tag = "f_tag";
            push!(t, f_tag, " = ", data, "[", tag.clone(), "]");

            for sub_type in body.sub_types.iter() {
                t.push_into(|t| {
                    push!(t, "if ", f_tag, " == ", sub_type.name().quoted(), ":");
                    nested!(t, "return ", &sub_type.name, ".decode(data)");
                });
            }

            push!(
                t,
                "raise Exception(",
                "bad type: ".quoted(),
                " + ",
                f_tag,
                ")"
            );

            Ok({
                let mut decode = Tokens::new();
                decode.push("@staticmethod");
                decode.push(toks!("def decode(", data, "):"));
                decode.nested(t.join_line_spacing());
                decode
            })
        }
    }

    fn process_service(&self, out: &mut Self::Out, body: &'el RpServiceBody) -> Result<()> {
        let mut type_body = Tokens::new();

        let mut extra: Vec<EndpointExtra> = Vec::new();

        for endpoint in &body.endpoints {
            let response_ty = if let Some(res) = endpoint.response.as_ref() {
                Some(("data", res.ty().decode("data".into())))
            } else {
                None
            };

            extra.push(EndpointExtra {
                name: endpoint.ident(),
                response_ty: response_ty,
            });
        }

        for g in &self.service_generators {
            g.generate(ServiceAdded {
                body: body,
                type_body: &mut type_body,
                extra: &extra,
            })?;
        }

        out.0.push(type_body);
        Ok(())
    }

    fn populate_files(&self) -> Result<BTreeMap<RpPackage, FileSpec<'el>>> {
        let mut enums = Vec::new();

        let mut files = self.do_populate_files(|decl| {
            if let core::RpDecl::Enum(ref body) = *decl {
                enums.push(body);
            }

            Ok(())
        })?;

        // Process picked up enums.
        // These are added to the end of the file to declare enums:
        // https://docs.python.org/3/library/enum.html
        for body in enums {
            if let Some(ref mut file_spec) = files.get_mut(&body.name.package) {
                file_spec.0.push(self.enum_variants(&body)?);
            } else {
                return Err(format!("missing file for package: {}", &body.name.package).into());
            }
        }

        Ok(files)
    }

    fn resolve_full_path(&self, package: &RpPackage) -> Result<RelativePathBuf> {
        let handle = self.handle();

        let mut full_path = RelativePathBuf::new();
        let mut iter = package.parts().peekable();

        while let Some(part) = iter.next() {
            full_path = full_path.join(part);

            if iter.peek().is_none() {
                continue;
            }

            if !handle.is_dir(&full_path) {
                debug!("+dir: {}", full_path.display());
                handle.create_dir_all(&full_path)?;
            }

            let init_path = full_path.join(INIT_PY);

            if !handle.is_file(&init_path) {
                debug!("+init: {}", init_path.display());
                handle.create(&init_path)?;
            }
        }

        full_path.set_extension(self.ext());
        Ok(full_path)
    }
}
