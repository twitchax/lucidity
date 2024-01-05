//! The `lucidity-macros` crate.  Provides the `job` macro.

#![warn(rustdoc::broken_intra_doc_links, rust_2018_idioms, clippy::all, missing_docs)]

use core::panic;
use std::str::FromStr;

use convert_case::{Casing, Case};
use proc_macro2::{TokenStream, Literal, TokenTree};
use quote::quote;
use syn::{ItemFn, Ident};

/// The `job` macro.
/// 
/// This macro creates a local and remote version of the function, as well as a local and remote async version of the function.
/// In addition, it creates the underlying [`AbstractProcess`] and [`Job`] types.
#[proc_macro_attribute]
pub fn job(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    job_inner(TokenStream::from(attr), TokenStream::from(item)).into()
}

fn job_inner(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse2::<ItemFn>(item).unwrap();

    // Parse code, and get identifiers.

    let name = &input.sig.ident;
    let vis = &input.vis;
    let return_block = &input.sig.output;
    let return_type = match return_block {
        syn::ReturnType::Default => quote! { () },
        syn::ReturnType::Type(_, ty) => quote! { #ty }
    };
    let name_pascal = Ident::new(&name.to_string().to_case(Case::Pascal), name.span());

    // Get argument information.

    let arguments = &input.sig.inputs;
    let arguments_names = arguments.iter().map(|arg| {
        match arg {
            syn::FnArg::Typed(pat_type) => {
                match &*pat_type.pat {
                    syn::Pat::Ident(ident) => {
                        let name = &ident.ident;
                        quote! { #name }
                    },
                    _ => panic!("Invalid argument pattern.")
                }
            },
            _ => panic!("Invalid argument pattern.")
        }
    });
    let arguments_types = arguments.iter().map(|arg| {
        match arg {
            syn::FnArg::Typed(pat_type) => {
                let ty = &pat_type.ty;
                quote! { #ty }
            },
            _ => panic!("Invalid argument pattern.")
        }
    });
    
    let call_arguments = quote! { #(#arguments_names),* };
    let arguments_types_list = if arguments.is_empty() {
        quote! { () }
    } else if arguments.len() == 1 {
        quote! { #(#arguments_types),* }
    }else {
        quote! { (#(#arguments_types),*) }
    };
    let option_return_type = quote! { Option<#return_type> };
    let arguments_args_tuple = if arguments.is_empty() {
        quote! {  }
    } else if arguments.len() == 1 {
        quote! { args }
    }else {
        let a = arguments.iter().enumerate().map(|(k, _)| {
            let s = Literal::usize_unsuffixed(k);
            quote! { args.#s }
        });

        quote! { #(#a),* }
    };
    let arguments_args_tuple_list = quote! { #arguments_args_tuple };

    // Names of generated identifiers.

    let service_name_ident = Ident::new(&format!("{}Service", name_pascal), name_pascal.span());
    let job_name_ident = Ident::new(&format!("{}Job", name_pascal), name_pascal.span());

    let local_fn_ident = Ident::new(&format!("{}_local", name), name.span());
    let remote_fn_ident = Ident::new(&format!("{}_remote", name), name.span());
    let local_async_fn_ident = Ident::new(&format!("{}_local_async", name), name.span());
    let remote_async_fn_ident = Ident::new(&format!("{}_remote_async", name), name.span());
    let remote_fanout_fn_ident = Ident::new(&format!("{}_remote_fanout", name), name.span());

    let get_ident = Ident::new(&format!("{}_get", name), name.span());
    let set_ident = Ident::new(&format!("{}_set", name), name.span());
    let try_get_ident = Ident::new(&format!("{}_try_get", name), name.span());
    let async_init_ident = Ident::new(&format!("{}_init_async", name), name.span());

    // Parse the attributes.
    
    let attr = attr.into_iter().collect::<Vec<_>>();
    let attr = attr.split(|t| {
        match t {
            TokenTree::Punct(punct) => {
                punct.as_char() == ','
            },
            _ => false
        }
    }).map(|tt| {
        let tt = tt.iter().collect::<Vec<_>>();
        tt.split(|t| {
            match t {
                TokenTree::Punct(punct) => {
                    punct.as_char() == '='
                },
                _ => false
            }
        }).flat_map(ToOwned::to_owned).collect::<Vec<_>>()
    }).filter_map(|v| {
        if v.len() != 2 {
            return None;
        }

        Some((v[0], v[1]))
    }).collect::<Vec<_>>();
    
    let mut init_retry_interval_ms = 100;
    let mut sync_retry_interval_ms = 100;
    let mut async_init_retry_interval_ms = 100;
    let mut async_get_retry_interval_ms = 100;
    let mut async_set_retry_interval_ms = 100;
    let mut shutdown_retry_interval_ms = 100;
    let mut fanout = Literal::from_str("\"roundrobin\"").unwrap();
    for (key, value) in attr {
        let key = key.to_string();
        let key = key.as_str();

        match key {
            "init_retry_interval_ms" => {
                let value = value.to_string();
                let value = value.as_str();

                match value.parse::<u64>() {
                    Ok(v) => {
                        init_retry_interval_ms = v;
                    },
                    Err(_) => panic!("Invalid attribute argument value `{}`.", value)
                }
            },
            "sync_retry_interval_ms" => {
                let value = value.to_string();
                let value = value.as_str();

                match value.parse::<u64>() {
                    Ok(v) => {
                        sync_retry_interval_ms = v;
                    },
                    Err(_) => panic!("Invalid attribute argument value `{}`.", value)
                }
            },
            "async_init_retry_interval_ms" => {
                let value = value.to_string();
                let value = value.as_str();

                match value.parse::<u64>() {
                    Ok(v) => {
                        async_init_retry_interval_ms = v;
                    },
                    Err(_) => panic!("Invalid attribute argument value `{}`.", value)
                }
            },
            "async_get_retry_interval_ms" => {
                let value = value.to_string();
                let value = value.as_str();

                match value.parse::<u64>() {
                    Ok(v) => {
                        async_get_retry_interval_ms = v;
                    },
                    Err(_) => panic!("Invalid attribute argument value `{}`.", value)
                }
            },
            "async_set_retry_interval_ms" => {
                let value = value.to_string();
                let value = value.as_str();

                match value.parse::<u64>() {
                    Ok(v) => {
                        async_set_retry_interval_ms = v;
                    },
                    Err(_) => panic!("Invalid attribute argument value `{}`.", value)
                }
            },
            "shutdown_retry_interval_ms" => {
                let value = value.to_string();
                let value = value.as_str();

                match value.parse::<u64>() {
                    Ok(v) => {
                        shutdown_retry_interval_ms = v;
                    },
                    Err(_) => panic!("Invalid attribute argument value `{}`.", value)
                }
            },
            "fanout" => {
                let value = value.to_string();
                let value = value.as_str();

                fanout = Literal::from_str(value).expect("The fanout was not a valid string.");
            },
            _ => panic!("Invalid attribute argument name `{}`.", key)
        }
    }

    // Get some special quotes.

    let config = quote! {
        let mut config = lucidity::lunatic::ProcessConfig::new().unwrap();
        config.set_can_spawn_processes(true);
        config.set_can_create_configs(true);
        config.set_can_compile_modules(true);
        config.set_max_fuel(u64::MAX);
        config.set_max_memory(u64::MAX);
    };

    let service = quote! {
        let service = loop {
            //println!("[{},{}] init > start", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
            match #service_name_ident::on_node(node).configure(&config).start_timeout((), std::time::Duration::from_millis(#init_retry_interval_ms)) {
                Ok(s) => {
                    //println!("[{},{}] init > end", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                    break s;
                },
                Err(e) => match e {
                    lucidity::lunatic::ap::StartupError::TimedOut => continue,
                    _ => panic!("Init error: {:#?}", e)
                }
            }
        };
    };

    // Generate the code.

    let gen = quote! {
        #input

        /// The generated "local" function.
        /// 
        /// This is a helper function for cases where you want to call the [`lucidity::job`] synchronously
        /// on the local node.
        /// 
        /// This function will block the current lunatic process until completion of the service process.
        /// 
        /// The sync method retry interval is defined by `sync_retry_interval_ms` (default 100ms).
        /// 
        /// The shutdown method retry interval is defined by `shutdown_retry_interval_ms` (default 100ms).
        /// 
        /// The service process is `shutdown` before completion of this call.
        #vis fn #local_fn_ident(#arguments) -> #return_type {
            use lucidity::lunatic::AbstractProcess;

            let node = lucidity::lunatic::host::node_id();
        
            #config
            
            #service
        
            // Get the result.
            let result = loop {
                //println!("[{},{}] get > start", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                if let Ok(r) = service.with_timeout(std::time::Duration::from_millis(#sync_retry_interval_ms)).#get_ident(#call_arguments) {
                    //println!("[{},{}] get > end", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                    break r;
                }
            };

            // Shutdown.
            loop {
                //println!("[{},{}] shutdown > start", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                if let Ok(_) = service.with_timeout(std::time::Duration::from_millis(#shutdown_retry_interval_ms)).shutdown() {
                    //println!("[{},{}] shutdown > end", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                    break;
                }
            }
        
            result
        }

        /// The generated "remote" function.
        /// 
        /// This is a helper function for cases where you want to call the [`lucidity::job`] synchronously
        /// on a random node in the distributed pool.
        /// 
        /// This function will block the current lunatic process until completion of the service process.
        /// 
        /// The sync method retry interval is defined by `sync_retry_interval_ms` (default 100ms).
        /// 
        /// The shutdown method retry interval is defined by `shutdown_retry_interval_ms` (default 100ms).
        /// 
        /// The service process is `shutdown` before completion of this call.
        #vis fn #remote_fn_ident(#arguments) -> #return_type {
            use lucidity::lunatic::AbstractProcess;
            use lucidity::rand::seq::SliceRandom;

            let nodes = lucidity::lunatic::distributed::nodes();
            let node = *nodes.choose(&mut lucidity::rand::thread_rng()).unwrap();
        
            #config
            
            #service
        
            // Get the result.
            let result = loop {
                //println!("[{},{}] get > start", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                if let Ok(r) = service.with_timeout(std::time::Duration::from_millis(#sync_retry_interval_ms)).#get_ident(#call_arguments) {
                    //println!("[{},{}] get > end", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                    break r;
                }
            };

            // Shutdown.
            loop {
                //println!("[{},{}] shutdown > start", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                if let Ok(_) = service.with_timeout(std::time::Duration::from_millis(#shutdown_retry_interval_ms)).shutdown() {
                    //println!("[{},{}] shutdown > end", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                    break;
                }
            }
        
            result
        }

        /// The generated "local async" function.
        /// 
        /// This is a helper function for cases where you want to call the [`lucidity::job`] asynchronously
        /// on the local node.
        /// 
        /// This function returns a [`Job`] that can be used to poll, or await, the result of the async process.
        /// 
        /// The async init method retry interval is defined by `async_init_retry_interval_ms` (default 100ms).
        /// 
        /// The spawned service process is `shutdown` when the returned [`Job`] is dropped.
        #vis fn #local_async_fn_ident(#arguments) -> #job_name_ident {
            use lucidity::lunatic::AbstractProcess;

            let node = lucidity::lunatic::host::node_id();

            #config
            
            #service

            // Get the result.
            let _ = loop {
                println!("[{},{}] async_init > start", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                if let Ok(r) = service.with_timeout(std::time::Duration::from_millis(#async_init_retry_interval_ms)).#async_init_ident(#call_arguments) {
                    println!("[{},{}] async_init > end", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                    break r;
                }
            };
            
            #job_name_ident(lucidity::Job {
                process: service
            })
        }

        /// The generated "remote async" function.
        /// 
        /// This is a helper function for cases where you want to call the [`lucidity::job`] asynchronously
        /// on a random node in the distributed pool.
        /// 
        /// This function returns a [`Job`] that can be used to poll, or await, the result of the async process.
        /// 
        /// The async init method retry interval is defined by `async_init_retry_interval_ms` (default 100ms).
        /// 
        /// The spawned service process is `shutdown` when the returned [`Job`] is dropped.
        #vis fn #remote_async_fn_ident(#arguments) -> #job_name_ident {
            use lucidity::lunatic::AbstractProcess;
            use lucidity::rand::seq::SliceRandom;

            let nodes = lucidity::lunatic::distributed::nodes();
            let node = *nodes.choose(&mut lucidity::rand::thread_rng()).unwrap();

            #config
            
            #service

            // Get the result.
            let _ = loop {
                //println!("[{},{}] async_init > start", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                if let Ok(r) = service.with_timeout(std::time::Duration::from_millis(#async_init_retry_interval_ms)).#async_init_ident(#call_arguments) {
                    //println!("[{},{}] async_init > end", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                    break r;
                }
            };
            
            #job_name_ident(lucidity::Job {
                process: service
            })
        }

        /// The generated "remote async fanout" function.
        /// 
        /// This is a helper function for cases where you want to call the [`lucidity::job`] asynchronously
        /// across a distributed set of machines.  Essentially, this is like a "rayon fanout", except that
        /// it fans across machines, rather than cores.
        /// 
        /// This functionality could be achieved using the other async methods, but this helper makes it easier to
        /// fanout, wait, and receive all of the results.
        /// 
        /// The async init method retry interval is defined by `async_init_retry_interval_ms` (default 100ms).
        /// 
        /// The async get method retry interval is defined by `async_get_retry_interval_ms` (default 100ms).
        /// 
        /// The shutdown method retry interval is defined by `shutdown_retry_interval_ms` (default 100ms).
        #vis fn #remote_fanout_fn_ident(args_list: Vec<#arguments_types_list>) -> Vec<#return_type> {
            use lucidity::lunatic::AbstractProcess;
            use lucidity::rand::seq::SliceRandom;
            
            #config
            
            let mut services = Vec::new();
            let nodes = lucidity::lunatic::distributed::nodes();

            let random = &mut lucidity::rand::thread_rng();

            for (k, args) in args_list.into_iter().enumerate() {
                let node = if #fanout == "roundrobin" {
                    let num_nodes = nodes.len();
                    nodes[k % num_nodes]
                } else {
                    // Default to random.
                    *nodes.choose(random).unwrap()
                };

                #service

                loop {
                    //println!("[{},{}] async_init > start", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                    if let Ok(_) = service.with_timeout(std::time::Duration::from_millis(#async_init_retry_interval_ms)).#async_init_ident(#arguments_args_tuple_list) {
                        //println!("[{},{}] async_init > end", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                        break;
                    }
                }

                services.push(service);
            }

            // Get all of the results.
            let mut results = vec![None; services.len()];
            loop {
                let mut done = true;

                for (k, service) in services.iter_mut().enumerate() {
                    if results[k].is_some() {
                        continue;
                    }

                    // Get the result.
                    let result = loop {
                        //println!("[{},{}] try_get > start", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                        if let Ok(r) = service.with_timeout(std::time::Duration::from_millis(#async_get_retry_interval_ms)).#try_get_ident() {
                            //println!("[{},{}] try_get > end", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                            break r;
                        }
                    };

                    if let Some(result) = result {
                        // Set the result.
                        results[k] = Some(result);

                        // Shutdown.
                        loop {
                            //println!("[{},{}] shutdown > start", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                            if let Ok(_) = service.with_timeout(std::time::Duration::from_millis(#shutdown_retry_interval_ms)).shutdown() {
                                //println!("[{},{}] shutdown > end", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                                break;
                            }
                        }
                    } else {
                        done = false;
                    }
                }

                if done {
                    break;
                }

                // Give some time for the processes to respond between loops.
                lucidity::lunatic::sleep(std::time::Duration::from_millis(#async_get_retry_interval_ms));
            }

            results.into_iter().map(|result| result.unwrap()).collect()
        }

        /// The generated [`AbstractProcess`] for the [`lucidity::job`].
        /// 
        /// This defines the proper methods to achieve synchronous, and asynchronous calls to a process
        /// that may be local or remote.  All of the generated functions make calls into this processes 
        /// request handlers.
        #vis struct #service_name_ident(#option_return_type);

        #[lucidity::lunatic::abstract_process(serializer = lucidity::lunatic::serializer::Bincode)]
        impl #service_name_ident {
            #[init]
            fn init(_: lucidity::lunatic::ap::Config<Self>, _: ()) -> Result<Self, ()> {
                //println!("[{},{}] init", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                Ok(Self(None))
            }

            #[terminate]
            fn terminate(self) {
                //println!("[{},{}] terminate", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
            }

            #[handle_link_death]
            fn handle_link_death(&self, _tag: lucidity::lunatic::Tag) {
                //println!("[{},{}] handle_link_death", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
            }

            #[handle_request]
            fn #get_ident(&self, #arguments) -> #return_type {
                //println!("[{},{}] get", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                #name(#call_arguments)
            }

            #[handle_request]
            fn #set_ident(&mut self, value: #return_type) {
                //println!("[{},{}] set", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                self.0 = Some(value);
            }

            #[handle_request]
            fn #try_get_ident(&self) -> #option_return_type {
                //println!("[{},{}] try_get", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                self.0
            }

            #[handle_request]
            fn #async_init_ident(&self, #arguments) {
                let parent: lucidity::lunatic::ap::ProcessRef<#service_name_ident> = unsafe { lucidity::lunatic::ap::ProcessRef::new(lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id()) };

                #config
                
                let _ = lucidity::lunatic::Process::spawn_link_config(&config, (parent, #call_arguments), |(parent, #call_arguments), _: lucidity::lunatic::Mailbox<()>| {
                    //println!("[{},{}] get_async", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());

                    let result = #name(#call_arguments);

                    loop {
                        println!("[{},{}] set > start", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());

                        if let Ok(_) = parent.with_timeout(std::time::Duration::from_millis(#async_set_retry_interval_ms)).#set_ident(result) {
                            println!("[{},{}] set > end", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                            break;
                        }
                    }
                });
            }
        }

        /// The [`Job`] type for the generated service.
        /// 
        /// This type is usually created with the [`lucidity::job`] macro on the async methods.
        /// The async methods are [`#local_async_fn_ident`] and [`#remote_async_fn_ident`],
        /// and they return this type, so that `try_get` and `await_get` can be called on it.
        /// 
        /// When this type is dropped, the underlying process is shutdown.
        #vis struct #job_name_ident(#vis lucidity::Job<#service_name_ident>);

        impl #job_name_ident {
            /// The `try_get` method on the generated [`Job`] type calls the service process to check if a value is ready.
            /// 
            /// This is generally used in some sort of loop, or context where multiple values need to be checked repeatedly.
            /// As the underlying lunatic runtime uses a message-based coroutine paradigm, this method acts as a helper to 
            /// synchronize across processes.
            #vis fn try_get(&self) -> #option_return_type {
                //println!("[{},{}] try_get_local", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                
                loop {
                    //println!("[{},{}] try_get > start", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                    if let Ok(r) = self.0.process.with_timeout(std::time::Duration::from_millis(#async_get_retry_interval_ms)).#try_get_ident() {
                        //println!("[{},{}] try_get > end", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                        return r;
                    }
                }
            }
        
            /// The `await_get` method on the generated [`Job`] type calls the service process repeatedly to check if a value is ready.
            /// 
            /// This is achieved by looping over `try_get` with a timeout of `async_get_retry_interval_ms` (default 100ms).
            /// As the underlying lunatic runtime uses a message-based coroutine paradigm, this method acts as a helper to 
            /// synchronize across processes.
            #vis fn await_get(&self) -> #return_type {
                //println!("[{},{}] await_get_local", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                
                loop {
                    println!("[{},{}] try_get_await > start", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                    if let Ok(r) = self.0.process.with_timeout(std::time::Duration::from_millis(#async_get_retry_interval_ms)).#try_get_ident() {
                        if let Some(r) = r {
                            println!("[{},{}] try_get_await > end", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                            return r;
                        } else {
                            println!("[{},{}] try_get_await > continue", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                            lucidity::lunatic::sleep(std::time::Duration::from_millis(#async_get_retry_interval_ms));
                        }
                    }
                }
            }
        }
    };

    gen
}

// Tests.

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_job() {
        let expected = quote! {
            fn pythagorean(num1: u32, num2: u32) -> f32 {
                ((num1 * num1 + num2 * num2) as f32).sqrt()
            }

            fn pythagorean_local(num1: u32, num2: u32) -> f32 {
                use lucidity::lunatic::AbstractProcess;

                let node = lucidity::lunatic::host::node_id();
            
                let mut config = lucidity::lunatic::ProcessConfig::new().unwrap();
                config.set_can_spawn_processes(true);
                config.set_can_create_configs(true);
                config.set_can_compile_modules(true);
                
                let service = PythagoreanService::on_node(node).configure(&config).start(()).unwrap();
            
                let result = service.pythagorean_get(num1, num2);
            
                service.shutdown();
            
                result
            }

            fn pythagorean_remote(num1: u32, num2: u32) -> f32 {
                use lucidity::lunatic::AbstractProcess;
                use lucidity::rand::seq::SliceRandom;

                let nodes = lucidity::lunatic::distributed::nodes();
                let node = *nodes.choose(&mut lucidity::rand::thread_rng()).unwrap();
            
                let mut config = lucidity::lunatic::ProcessConfig::new().unwrap();
                config.set_can_spawn_processes(true);
                config.set_can_create_configs(true);
                config.set_can_compile_modules(true);
                
                let service = PythagoreanService::on_node(node).configure(&config).start(()).unwrap();
            
                let result = service.pythagorean_get(num1, num2);
            
                service.shutdown();
            
                result
            }

            fn pythagorean_local_async(num1: u32, num2: u32) -> PythagoreanJob {
                use lucidity::lunatic::AbstractProcess;

                let node = lucidity::lunatic::host::node_id();

                let mut config = lucidity::lunatic::ProcessConfig::new().unwrap();
                config.set_can_spawn_processes(true);
                config.set_can_create_configs(true);
                config.set_can_compile_modules(true);
                
                let service = PythagoreanService::on_node(node).configure(&config).start(()).unwrap();

                service.pythagorean_get_async(num1, num2);
                
                PythagoreanJob(lucidity::Job {
                    process: service
                })
            }

            fn pythagorean_remote_async(num1: u32, num2: u32) -> PythagoreanJob {
                use lucidity::lunatic::AbstractProcess;
                use lucidity::rand::seq::SliceRandom;

                let nodes = lucidity::lunatic::distributed::nodes();
                let node = *nodes.choose(&mut lucidity::rand::thread_rng()).unwrap();

                let mut config = lucidity::lunatic::ProcessConfig::new().unwrap();
                config.set_can_spawn_processes(true);
                config.set_can_create_configs(true);
                config.set_can_compile_modules(true);
                
                let service = PythagoreanService::on_node(node).configure(&config).start(()).unwrap();

                service.pythagorean_get_async(num1, num2);
                
                PythagoreanJob(lucidity::Job {
                    process: service
                })
            }

            fn pythagorean_remote_async_fanout(args_list: Vec<(u32, u32)>, interval: std::time::Duration) -> Vec<f32> {
                use lucidity::lunatic::AbstractProcess;
                use lucidity::rand::seq::SliceRandom;

                let mut config = lucidity::lunatic::ProcessConfig::new().unwrap();
                config.set_can_spawn_processes(true);
                config.set_can_create_configs(true);
                config.set_can_compile_modules(true);
                
                let mut services = Vec::new();
                let nodes = lucidity::lunatic::distributed::nodes();

                for args in args_list {
                    let node = *nodes.choose(&mut lucidity::rand::thread_rng()).unwrap();
                    
                    let service = PythagoreanService::on_node(node).configure(&config).start(()).unwrap();
                    service.pythagorean_get_async(args.0, args.1);
                    services.push(service);
                }

                let mut results = vec![None; services.len()];
                loop {
                    let mut done = true;

                    for (k, service) in services.iter_mut().enumerate() {
                        if let Some(result) = service.pythagorean_try_get() {
                            results[k] = Some(result);
                        } else {
                            done = false;
                        }
                    }

                    if done {
                        break;
                    }

                    lucidity::lunatic::sleep(interval);
                }

                results.into_iter().map(|result| result.unwrap()).collect()
            }

            struct PythagoreanService(Option<f32>);

            #[lucidity::lunatic::abstract_process(serializer = lucidity::lunatic::serializer::Bincode)]
            impl PythagoreanService {
                #[init]
                fn init(_: lucidity::lunatic::ap::Config<Self>, _: ()) -> Result<Self, ()> {
                    println!("[{},{}] init", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                    Ok(Self(None))
                }

                #[terminate]
                fn terminate(self) {
                    println!("[{},{}] terminate", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                }

                #[handle_link_death]
                fn handle_link_death(&self, _tag: lucidity::lunatic::Tag) {
                    println!("[{},{}] handle_link_death", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                }

                #[handle_request]
                fn pythagorean_get(&self, num1: u32, num2: u32) -> f32 {
                    println!("[{},{}] get", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                    pythagorean(num1, num2)
                }

                #[handle_request]
                fn pythagorean_set(&mut self, value: f32) {
                    println!("[{},{}] set", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                    self.0 = Some(value);
                }

                #[handle_request]
                fn pythagorean_try_get(&self) -> Option<f32> {
                    println!("[{},{}] try_get", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                    self.0
                }

                #[handle_message]
                fn pythagorean_get_async(&mut self, num1: u32, num2: u32) {
                    let parent: lucidity::lunatic::ap::ProcessRef<PythagoreanService> = unsafe { lucidity::lunatic::ap::ProcessRef::new(lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id()) };
                    
                    let _ = lucidity::lunatic::Process::spawn_link((parent, num1, num2), |(parent, num1, num2), _: lucidity::lunatic::Mailbox<()>| {
                        println!("[{},{}] get_async", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                        let result = pythagorean(num1, num2);
                        parent.pythagorean_set(result);
                    });
                }
            }

            struct PythagoreanJob(lucidity::Job<PythagoreanService>);

            impl PythagoreanJob {
                fn try_get(&self) -> Option<f32> {
                    println!("[{},{}] try_get_local", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                    
                    self.0.process.pythagorean_try_get()
                }
            
                fn await_get(&self, interval: std::time::Duration) -> f32 {
                    println!("[{},{}] await_get_local", lucidity::lunatic::host::node_id(), lucidity::lunatic::host::process_id());
                    
                    loop {
                        if let Some(result) = self.0.process.pythagorean_try_get() {
                            return result;
                        }
            
                        lucidity::lunatic::sleep(interval);
                    }
                }
            }
        };

        let input = quote! {
            fn pythagorean(num1: u32, num2: u32) -> f32 {
                ((num1 * num1 + num2 * num2) as f32).sqrt()
            }
        };

        let input = job_inner(TokenStream::new(), input).to_string();

        assert_eq!(expected.to_string(), input.to_string());
    }

    #[test]
    fn test_fail() {
        let input = quote! {
            fn foo(num1: u32) -> f32 {
                0.0
            }
        };

        let input = job_inner(TokenStream::new(), input).to_string();

        assert_eq!("".to_string(), input.to_string());
    }
}