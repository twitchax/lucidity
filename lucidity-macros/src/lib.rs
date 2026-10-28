//! The `lucidity-macros` crate.  Provides the `job` macro.

#![warn(rustdoc::broken_intra_doc_links, rust_2018_idioms, clippy::all, missing_docs)]

use convert_case::{Case, Casing};
use proc_macro2::{Literal, TokenStream, TokenTree};
use quote::quote;
use syn::{Ident, ItemFn};

/// Attributes from before jobs stopped polling; they're rejected with an explanation rather than silently ignored.
const REMOVED_ATTRIBUTES: [&str; 6] = [
    "init_retry_interval_ms",
    "sync_retry_interval_ms",
    "async_init_retry_interval_ms",
    "async_get_retry_interval_ms",
    "async_set_retry_interval_ms",
    "shutdown_retry_interval_ms",
];

/// Turns a function into a job that can run in its own process, on this node or another one.
///
/// See the crate documentation for the generated functions and the accepted attributes.
#[proc_macro_attribute]
pub fn job(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    job_inner(TokenStream::from(attr), TokenStream::from(item)).into()
}

fn job_inner(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse2::<ItemFn>(item).unwrap();

    // Parse code, and get identifiers.

    let name = &input.sig.ident;
    let vis = &input.vis;
    let return_type = match &input.sig.output {
        syn::ReturnType::Default => quote! { () },
        syn::ReturnType::Type(_, ty) => quote! { #ty },
    };
    let name_pascal = Ident::new(&name.to_string().to_case(Case::Pascal), name.span());

    // Get argument information.

    let arguments = &input.sig.inputs;
    let argument_names = arguments
        .iter()
        .map(|arg| match arg {
            syn::FnArg::Typed(pat_type) => match &*pat_type.pat {
                syn::Pat::Ident(ident) => ident.ident.clone(),
                _ => panic!("Invalid argument pattern."),
            },
            _ => panic!("Invalid argument pattern."),
        })
        .collect::<Vec<_>>();
    let argument_types = arguments
        .iter()
        .map(|arg| match arg {
            syn::FnArg::Typed(pat_type) => &pat_type.ty,
            _ => panic!("Invalid argument pattern."),
        })
        .collect::<Vec<_>>();

    // The arguments travel to the job's process as one value: `()`, the single argument, or a tuple of them.  This is
    // also the element type of the fanout `Vec`.
    let (args_type, args_value) = match argument_names.len() {
        0 => (quote! { () }, quote! { () }),
        1 => {
            let (ty, name) = (argument_types[0], &argument_names[0]);
            (quote! { #ty }, quote! { #name })
        }
        _ => (quote! { (#(#argument_types),*) }, quote! { (#(#argument_names),*) }),
    };

    // Names of generated identifiers.

    let job_name_ident = Ident::new(&format!("{}Job", name_pascal), name_pascal.span());

    let local_fn_ident = Ident::new(&format!("{}_local", name), name.span());
    let remote_fn_ident = Ident::new(&format!("{}_remote", name), name.span());
    let local_async_fn_ident = Ident::new(&format!("{}_local_async", name), name.span());
    let remote_async_fn_ident = Ident::new(&format!("{}_remote_async", name), name.span());
    let remote_fanout_fn_ident = Ident::new(&format!("{}_remote_fanout", name), name.span());

    // Parse the attributes.

    let attr = attr.into_iter().collect::<Vec<_>>();
    let attr = attr
        .split(|t| match t {
            TokenTree::Punct(punct) => punct.as_char() == ',',
            _ => false,
        })
        .map(|tt| {
            let tt = tt.iter().collect::<Vec<_>>();
            tt.split(|t| match t {
                TokenTree::Punct(punct) => punct.as_char() == '=',
                _ => false,
            })
            .flat_map(ToOwned::to_owned)
            .collect::<Vec<_>>()
        })
        .filter_map(|v| {
            if v.len() != 2 {
                return None;
            }

            Some((v[0], v[1]))
        })
        .collect::<Vec<_>>();

    // Set the attributes.

    let mut memory = 100u64 * 1024 * 1024;
    let mut fuel = quote! { None };
    let mut fanout = quote! { lucidity::Fanout::RoundRobin };
    for (key, value) in attr {
        let key = key.to_string();
        let key = key.as_str();
        let value = value.to_string();
        let value = value.as_str();

        match key {
            "memory" => match value.parse::<u64>() {
                Ok(v) => memory = v,
                Err(_) => panic!("Invalid attribute argument value `{}`.", value),
            },
            "fuel" => match value.parse::<u64>() {
                Ok(v) => {
                    let v = Literal::u64_unsuffixed(v);
                    fuel = quote! { Some(#v) };
                }
                Err(_) => panic!("Invalid attribute argument value `{}`.", value),
            },
            "fanout" => match value {
                "\"roundrobin\"" => fanout = quote! { lucidity::Fanout::RoundRobin },
                "\"random\"" => fanout = quote! { lucidity::Fanout::Random },
                _ => panic!("Invalid fanout `{}`: expected \"roundrobin\" or \"random\".", value),
            },
            _ if REMOVED_ATTRIBUTES.contains(&key) => panic!("`{}` was removed: jobs no longer poll, so there is nothing to retry.", key),
            _ => panic!("Invalid attribute argument name `{}`.", key),
        }
    }

    // Generate the code.

    quote! {
        #input

        /// The [`lucidity::JobFn`] for the generated job functions, and the type parameter of the [`lucidity::Job`]s
        /// returned by the async ones.
        #vis struct #job_name_ident;

        impl lucidity::JobFn for #job_name_ident {
            type Args = #args_type;
            type Output = #return_type;

            const NAME: &'static str = stringify!(#name);
            const MAX_MEMORY: u64 = #memory;
            const MAX_FUEL: Option<u64> = #fuel;

            fn call(#args_value: Self::Args) -> Self::Output {
                #name(#(#argument_names),*)
            }
        }

        /// The generated "local" function.
        ///
        /// Runs the job in a new process on this node, and blocks the current process until it finishes.
        ///
        /// Panics if the job's process dies.
        #vis fn #local_fn_ident(#arguments) -> #return_type {
            #local_async_fn_ident(#(#argument_names),*).await_get()
        }

        /// The generated "remote" function.
        ///
        /// Runs the job in a new process on a random node in the distributed pool, and blocks the current process until
        /// it finishes.
        ///
        /// Panics if the job's process dies, or if no other nodes are connected.
        #vis fn #remote_fn_ident(#arguments) -> #return_type {
            #remote_async_fn_ident(#(#argument_names),*).await_get()
        }

        /// The generated "local async" function.
        ///
        /// Runs the job in a new process on this node, and returns a [`lucidity::Job`] to get the result from with
        /// `try_get` or `await_get`.  Dropping the job before it finishes cancels it.
        #vis fn #local_async_fn_ident(#arguments) -> lucidity::Job<#job_name_ident> {
            lucidity::Job::spawn(lucidity::local_node(), #args_value)
        }

        /// The generated "remote async" function.
        ///
        /// Runs the job in a new process on a random node in the distributed pool, and returns a [`lucidity::Job`] to get
        /// the result from with `try_get` or `await_get`.  Dropping the job before it finishes cancels it.
        ///
        /// Panics if no other nodes are connected.
        #vis fn #remote_async_fn_ident(#arguments) -> lucidity::Job<#job_name_ident> {
            lucidity::Job::spawn(lucidity::random_node(), #args_value)
        }

        /// The generated "remote fanout" function.
        ///
        /// Runs one job per element of `args_list` across the distributed pool (see the `fanout` attribute), and blocks
        /// the current process until all of them finish.  The results are in the same order as `args_list`.
        ///
        /// Panics if any job's process dies, or if no other nodes are connected.
        #vis fn #remote_fanout_fn_ident(args_list: Vec<#args_type>) -> Vec<#return_type> {
            lucidity::fanout::<#job_name_ident>(args_list, #fanout)
        }
    }
}

// Tests.

#[cfg(test)]
mod tests {
    use super::*;

    fn expand(attr: TokenStream) -> String {
        let input = quote! {
            fn pythagorean(num1: u32, num2: u32) -> f32 {
                ((num1 * num1 + num2 * num2) as f32).sqrt()
            }
        };

        job_inner(attr, input).to_string()
    }

    #[test]
    fn test_job() {
        let output = expand(TokenStream::new());

        for generated in [
            "pythagorean_local",
            "pythagorean_remote",
            "pythagorean_local_async",
            "pythagorean_remote_async",
            "pythagorean_remote_fanout",
            "struct PythagoreanJob",
        ] {
            assert!(output.contains(generated), "missing `{generated}` in {output}");
        }
        assert!(output.contains("type Args = (u32 , u32)"), "{output}");
        assert!(output.contains("const MAX_FUEL : Option < u64 > = None"), "{output}");
        assert!(output.contains("lucidity :: Fanout :: RoundRobin"), "{output}");
    }

    #[test]
    fn test_job_attributes() {
        let output = expand(quote! { memory = 1024, fuel = 50, fanout = "random" });

        assert!(output.contains("const MAX_MEMORY : u64 = 1024"), "{output}");
        assert!(output.contains("const MAX_FUEL : Option < u64 > = Some (50)"), "{output}");
        assert!(output.contains("lucidity :: Fanout :: Random"), "{output}");
    }

    #[test]
    fn test_job_single_and_no_arguments() {
        let single = job_inner(TokenStream::new(), quote! { fn square(a: u32) -> u32 { a * a } }).to_string();
        assert!(single.contains("type Args = u32"), "{single}");

        let none = job_inner(TokenStream::new(), quote! { fn ping() {} }).to_string();
        assert!(none.contains("type Args = ()"), "{none}");
        assert!(none.contains("type Output = ()"), "{none}");
    }

    #[test]
    #[should_panic(expected = "`sync_retry_interval_ms` was removed")]
    fn test_job_removed_attribute() {
        expand(quote! { sync_retry_interval_ms = 10 });
    }

    #[test]
    #[should_panic(expected = "Invalid fanout")]
    fn test_job_invalid_fanout() {
        expand(quote! { fanout = "sideways" });
    }
}
