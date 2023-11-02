#![deny(clippy::all)]

#[macro_use]
extern crate napi_derive;

use napi::{
    bindgen_prelude::*,
    threadsafe_function::{ThreadSafeCallContext, ThreadsafeFunction, ThreadsafeFunctionCallMode},
};

#[napi(object)]
#[derive(Debug)]
pub struct Options {
    pub ignore_patterns: Option<Vec<String>>,
}

#[napi]
pub fn watch(path: String, options: Options, callback: JsFunction) -> Result<()> {
    let tsfn: ThreadsafeFunction<(String, Vec<String>)> = callback.create_threadsafe_function(
        0,
        |ctx: ThreadSafeCallContext<(String, Vec<String>)>| {
            let mut ret = vec![ctx.env.create_string(&*ctx.value.0)?];
            for path in ctx.value.1.iter() {
                ret.push(ctx.env.create_string(&*path)?);
            }
            Ok(ret)
        },
    )?;
    println!("D: watching {}", path);
    println!("D: with opts: {:?}", options);

    watcher::watch(
        path,
        options.ignore_patterns.iter().flatten().map(|s| s.as_str()),
        move |event| {
            tsfn.call(
                Ok((
                    event.kind().to_owned(),
                    event
                        .paths()
                        .into_iter()
                        .map(|p| p.to_str().unwrap().to_owned())
                        .collect(),
                )),
                ThreadsafeFunctionCallMode::Blocking,
            );
        },
    )
    .unwrap();

    Ok(())
}

#[napi]
pub fn close() -> Result<()> {
    watcher::close().unwrap();
    Ok(())
}
