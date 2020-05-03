/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * This file is part of M3 (Microkernel-based SysteM for Heterogeneous Manycores).
 *
 * M3 is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License version 2 as
 * published by the Free Software Foundation.
 *
 * M3 is distributed in the hope that it will be useful, but
 * WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * General Public License version 2 for more details.
 */

use m3::col::{String, ToString, Vec};
use m3::errors::{Code, Error};
use m3::goff;
use m3::rc::Rc;

use config;

struct ConfigParser {
    chars: Vec<char>,
    pos: usize,
}

impl ConfigParser {
    fn new(xml: &str) -> Self {
        ConfigParser {
            chars: xml.chars().collect(),
            pos: 0,
        }
    }

    fn get(&mut self) -> Result<char, Error> {
        if self.pos < self.chars.len() {
            let idx = self.pos;
            self.pos += 1;
            Ok(self.chars[idx])
        }
        else {
            Err(Error::new(Code::InvArgs))
        }
    }

    fn put(&mut self) -> Option<char> {
        if self.pos > 0 {
            self.pos -= 1;
            Some(self.chars[self.pos])
        }
        else {
            None
        }
    }

    fn get_no_ws(&mut self) -> Result<char, Error> {
        loop {
            let c = self.get()?;
            if c.is_whitespace() {
                continue;
            }
            break Ok(c);
        }
    }

    fn consume(&mut self, c: char) -> Result<(), Error> {
        let nc = self.get_no_ws()?;
        if nc != c {
            Err(Error::new(Code::InvArgs))
        }
        else {
            Ok(())
        }
    }

    fn parse_ident(&mut self, delim: char) -> Result<String, Error> {
        let mut name_buf = String::new();
        let first = self.get_no_ws()?;
        name_buf.push(first);

        while let Ok(c) = self.get() {
            if c == delim || c.is_whitespace() {
                break;
            }

            name_buf.push(c);
        }
        Ok(name_buf)
    }

    fn parse_arg(&mut self) -> Result<Option<(String, String)>, Error> {
        let first = self.get_no_ws()?;
        self.put();
        if first == '>' || first == '/' {
            return Ok(None);
        }

        let name = self.parse_ident('=')?;
        self.consume('"')?;

        let mut val_buf = String::new();
        while let Ok(c) = self.get() {
            if c == '"' {
                break;
            }

            val_buf.push(c);
        }
        Ok(Some((name, val_buf)))
    }

    fn ignore_args(&mut self) -> Result<(), Error> {
        loop {
            match self.parse_arg()? {
                None => break Ok(()),
                Some(_) => continue,
            }
        }
    }

    fn parse_tag_name(&mut self) -> Result<Option<String>, Error> {
        self.consume('<')?;

        let mut name_buf = String::new();
        let first = self.get_no_ws()?;

        if first == '/' {
            while let Some(n) = self.put() {
                if n == '<' {
                    return Ok(None);
                }
            }
        }
        name_buf.push(first);

        while let Ok(c) = self.get() {
            if c.is_whitespace() {
                break;
            }
            if c == '>' || c == '/' {
                self.put();
                break;
            }

            name_buf.push(c);
        }

        Ok(Some(name_buf))
    }
}

pub(crate) fn parse(xml: &str, restrict: bool) -> Result<config::Config, Error> {
    let mut p = ConfigParser::new(xml);
    let mut cfg = config::Config { doms: Vec::new() };

    parse_open_tag(&mut p, "config", |_| Err(Error::new(Code::InvArgs)))?;

    parse_simple_element(&mut p, "kernel", |p| p.ignore_args())?;

    parse_domains(&mut p, &mut cfg, restrict)?;

    parse_close_tag(&mut p, "config")?;
    Ok(cfg)
}

fn parse_domains(
    p: &mut ConfigParser,
    cfg: &mut config::Config,
    restrict: bool,
) -> Result<(), Error> {
    while let Some(tag) = p.parse_tag_name()? {
        if tag != "dom" {
            return Err(Error::new(Code::InvArgs));
        }

        p.consume('>')?;

        cfg.doms.push(parse_domain(p, restrict)?);

        parse_close_tag(p, "dom")?;
    }
    Ok(())
}

fn parse_domain(p: &mut ConfigParser, restrict: bool) -> Result<config::Domain, Error> {
    let mut group = config::Domain::default();
    while let Some(tag) = p.parse_tag_name()? {
        if tag != "app" {
            return Err(Error::new(Code::InvArgs));
        }

        group.apps.push(Rc::new(parse_app(p, restrict)?));
    }
    Ok(group)
}

fn parse_app(p: &mut ConfigParser, restrict: bool) -> Result<config::AppConfig, Error> {
    let mut app = config::AppConfig::default();
    app.restrict = restrict;

    loop {
        match p.parse_arg()? {
            None => break,
            Some((n, v)) => match n.as_ref() {
                "args" => {
                    for (i, a) in v.split_whitespace().enumerate() {
                        if i == 0 {
                            app.name = a.to_string();
                        }
                        app.args.push(a.to_string());
                    }
                },
                "usermem" => app.user_mem = Some(parse_size(&v)?),
                "kernmem" => app.kern_mem = Some(parse_size(&v)?),
                "eps" => app.eps = Some(v.parse::<u32>().map_err(|_| Error::new(Code::InvArgs))?),
                "daemon" => app.daemon = parse_bool(&v)?,
                _ => return Err(Error::new(Code::InvArgs)),
            },
        }
    }

    let nc = p.get_no_ws()?;
    if nc == '/' {
        p.consume('>')?;
        Ok(app)
    }
    else if nc == '>' {
        while let Some(tag) = p.parse_tag_name()? {
            match tag.as_ref() {
                "sess" => app.sessions.push(parse_session(p)?),
                "serv" => app.services.push(parse_service(p)?),
                "physmem" => app.phys_mems.push(parse_physmem(p)?),
                "pes" => app.pes.push(parse_pe(p)?),
                "sem" => app.sems.push(parse_sem(p)?),
                _ => return Err(Error::new(Code::InvArgs)),
            }

            p.consume('/')?;
            p.consume('>')?;
        }
        parse_close_tag(p, "app")?;
        Ok(app)
    }
    else {
        Err(Error::new(Code::InvArgs))
    }
}

fn parse_physmem(p: &mut ConfigParser) -> Result<config::PhysMemDesc, Error> {
    let mut phys = 0;
    let mut size = 0;
    loop {
        match p.parse_arg()? {
            None => break,
            Some((n, v)) => match n.as_ref() {
                "addr" => phys = parse_addr(&v)?,
                "size" => size = parse_size(&v)? as goff,
                _ => return Err(Error::new(Code::InvArgs)),
            },
        }
    }
    Ok(config::PhysMemDesc::new(phys, size))
}

fn parse_service(p: &mut ConfigParser) -> Result<config::ServiceDesc, Error> {
    let mut lname = String::new();
    let mut gname = String::new();
    loop {
        match p.parse_arg()? {
            None => break,
            Some((n, v)) => match n.as_ref() {
                "name" => {
                    lname = v.clone();
                    gname = v
                },
                "lname" => lname = v,
                "gname" => gname = v,
                _ => return Err(Error::new(Code::InvArgs)),
            },
        }
    }
    Ok(config::ServiceDesc::new(lname, gname))
}

fn parse_session(p: &mut ConfigParser) -> Result<config::SessionDesc, Error> {
    let mut lname = String::new();
    let mut serv = String::new();
    let mut arg = String::new();
    loop {
        match p.parse_arg()? {
            None => break,
            Some((n, v)) => match n.as_ref() {
                "name" => {
                    lname = v.clone();
                    serv = v
                },
                "lname" => lname = v,
                "gname" => serv = v,
                "args" => arg = v,
                _ => return Err(Error::new(Code::InvArgs)),
            },
        }
    }
    Ok(config::SessionDesc::new(lname, serv, arg))
}

fn parse_pe(p: &mut ConfigParser) -> Result<config::PEDesc, Error> {
    let mut ty = String::new();
    let mut count = 1;
    let mut optional = false;
    loop {
        match p.parse_arg()? {
            None => break,
            Some((n, v)) => match n.as_ref() {
                "type" => ty = v,
                "count" => count = v.parse::<u32>().map_err(|_| Error::new(Code::InvArgs))?,
                "optional" => optional = parse_bool(&v)?,
                _ => return Err(Error::new(Code::InvArgs)),
            },
        }
    }
    Ok(config::PEDesc::new(ty, count, optional))
}

fn parse_sem(p: &mut ConfigParser) -> Result<config::SemDesc, Error> {
    let mut lname = String::new();
    let mut gname = String::new();
    loop {
        match p.parse_arg()? {
            None => break,
            Some((n, v)) => match n.as_ref() {
                "name" => {
                    lname = v.clone();
                    gname = v
                },
                "lname" => lname = v,
                "gname" => gname = v,
                _ => return Err(Error::new(Code::InvArgs)),
            },
        }
    }
    Ok(config::SemDesc::new(lname, gname))
}

fn parse_open_tag<A>(p: &mut ConfigParser, name: &str, mut args: A) -> Result<bool, Error>
where
    A: FnMut(&mut ConfigParser) -> Result<(), Error>,
{
    p.consume('<')?;

    let mut name_buf = String::new();
    let first = p.get_no_ws()?;
    name_buf.push(first);

    let closed = loop {
        let mut c = p.get()?;
        if c == '>' || c == '/' || c.is_whitespace() {
            if name_buf != name {
                return Err(Error::new(Code::InvArgs));
            }
            if c.is_whitespace() {
                args(p)?;
                c = p.get_no_ws()?;
            }
            break c == '/';
        }

        name_buf.push(c);
    };

    if closed {
        p.consume('>')?;
        Ok(false)
    }
    else {
        Ok(true)
    }
}

fn parse_close_tag(p: &mut ConfigParser, name: &str) -> Result<(), Error> {
    p.consume('<')?;
    p.consume('/')?;

    let tname = p.parse_ident('>')?;
    if tname != name {
        Err(Error::new(Code::InvArgs))
    }
    else {
        Ok(())
    }
}

fn parse_simple_element<A>(p: &mut ConfigParser, name: &str, args: A) -> Result<(), Error>
where
    A: FnMut(&mut ConfigParser) -> Result<(), Error>,
{
    if parse_open_tag(p, name, args)? {
        parse_close_tag(p, name)
    }
    else {
        Ok(())
    }
}

fn parse_addr(s: &str) -> Result<goff, Error> {
    if s.starts_with("0x") {
        goff::from_str_radix(&s[2..], 16)
    }
    else {
        s.parse::<goff>()
    }
    .map_err(|_| Error::new(Code::InvArgs))
}

fn parse_size(s: &str) -> Result<usize, Error> {
    let mul = match s.chars().last() {
        Some(c) if c >= '0' && c <= '9' => 1,
        Some('k') | Some('K') => 1024,
        Some('m') | Some('M') => 1024 * 1024,
        Some('g') | Some('G') => 1024 * 1024 * 1024,
        _ => return Err(Error::new(Code::InvArgs)),
    };
    let num = match mul {
        1 => s.parse::<usize>(),
        _ => s[0..s.len() - 1].parse::<usize>(),
    }
    .map_err(|_| Error::new(Code::InvArgs))?;
    Ok(num * mul)
}

fn parse_bool(s: &str) -> Result<bool, Error> {
    match s {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => {
            let val = s.parse::<u32>().map_err(|_| Error::new(Code::InvArgs))?;
            Ok(val == 1)
        },
    }
}
