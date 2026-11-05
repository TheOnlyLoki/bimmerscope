//! The description block: `KEY:value` lines written by the BEST/2 compiler.
//! Global lines (`ECU:`, `ORIGIN:`, ...) come first, then one group per job
//! starting with `JOBNAME:`.

#[derive(Debug, Clone, Default)]
pub struct Param {
    pub name: String,
    pub ty: String,
    pub comments: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct JobDoc {
    pub name: String,
    pub comments: Vec<String>,
    pub args: Vec<Param>,
    pub results: Vec<Param>,
}

#[derive(Debug, Clone, Default)]
pub struct Description {
    /// Global `KEY:value` pairs in file order.
    pub global: Vec<(String, String)>,
    pub jobs: Vec<JobDoc>,
}

impl Description {
    pub fn parse(text: &str) -> Self {
        let mut d = Self::default();
        for line in text.lines() {
            let line = line.trim_end_matches('\r');
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            let value = value.to_string();
            let key = key.to_ascii_uppercase();
            if key == "JOBNAME" {
                d.jobs.push(JobDoc { name: value, ..Default::default() });
                continue;
            }
            let Some(job) = d.jobs.last_mut() else {
                d.global.push((key, value));
                continue;
            };
            match key.as_str() {
                "JOBCOMMENT" => job.comments.push(value),
                "ARG" => job.args.push(Param { name: value, ..Default::default() }),
                "RESULT" => job.results.push(Param { name: value, ..Default::default() }),
                "ARGTYPE" | "ARGCOMMENT" | "RESULTTYPE" | "RESULTCOMMENT" => {
                    let list = if key.starts_with("ARG") { &mut job.args } else { &mut job.results };
                    if let Some(p) = list.last_mut() {
                        if key.ends_with("TYPE") {
                            p.ty = value;
                        } else {
                            p.comments.push(value);
                        }
                    }
                }
                _ => {}
            }
        }
        d
    }

    /// First value of a global key, e.g. `ECU` or `ORIGIN`.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.global
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }

    /// All `ECUCOMMENT` lines joined.
    pub fn ecu_comment(&self) -> String {
        self.global
            .iter()
            .filter(|(k, _)| k == "ECUCOMMENT")
            .map(|(_, v)| v.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn job(&self, name: &str) -> Option<&JobDoc> {
        self.jobs.iter().find(|j| j.name.eq_ignore_ascii_case(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_globals_and_jobs() {
        let d = Description::parse(
            "ECU:JBBF\nECUCOMMENT:SGBD of JBBFE\nJOBNAME:IDENT\nJOBCOMMENT:Identdaten\n\
             ARG:X\nARGTYPE:int\nRESULT:JOB_STATUS\nRESULTTYPE:string\nRESULTCOMMENT:OKAY\n",
        );
        assert_eq!(d.get("ecu"), Some("JBBF"));
        assert_eq!(d.ecu_comment(), "SGBD of JBBFE");
        let j = d.job("ident").unwrap();
        assert_eq!(j.comments, ["Identdaten"]);
        assert_eq!(j.args[0].ty, "int");
        assert_eq!(j.results[0].comments, ["OKAY"]);
    }
}
