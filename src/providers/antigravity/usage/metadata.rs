use std::collections::{HashMap, HashSet, VecDeque};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use rusqlite::Connection;

use crate::usage::pricing::RequestTokens;

pub(super) struct Database {
    pub(super) calls: Vec<Call>,
    pub(super) models: HashMap<u64, Option<String>>,
}

pub(super) fn decode_database(
    connection: &Connection,
    fallback: DateTime<Utc>,
) -> Result<Database> {
    let mut tables = connection.prepare("SELECT name FROM sqlite_master WHERE type = 'table'")?;
    let tables = tables
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<HashSet<_>>>()?;
    let mut trajectory_time = None;
    if tables.contains("trajectory_metadata_blob") {
        let mut statement = connection.prepare("SELECT data FROM trajectory_metadata_blob")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            trajectory_time = Fields::decode(row.get_ref(0)?.as_blob()?)?
                .nested(2)?
                .timestamp();
            if trajectory_time.is_some() {
                break;
            }
        }
    }
    let mut steps = if tables.contains("steps") {
        read_metadata(
            connection,
            "SELECT idx, metadata FROM steps ORDER BY rowid",
            true,
        )?
    } else {
        Vec::new()
    };
    let mut generations = if tables.contains("gen_metadata") {
        read_metadata(
            connection,
            "SELECT idx, data FROM gen_metadata ORDER BY rowid",
            false,
        )?
    } else {
        Vec::new()
    };
    let mut models = HashMap::new();
    let generation_models = generations
        .iter()
        .map(|(idx, entry)| (*idx, &entry.model))
        .collect::<HashMap<_, _>>();
    for (_, entry) in steps.iter().chain(&generations) {
        if entry.model.id > 0 && is_concrete_model(&entry.model.name) {
            remember_model(&mut models, entry.model.id, Some(&entry.model.name));
        }
    }
    let fallback = trajectory_time.map_or((fallback, 0), |at| (at, 1));
    fill_calls(&mut steps, &generation_models, fallback);
    drop(generation_models);
    fill_calls(&mut generations, &HashMap::new(), fallback);
    let steps = steps
        .into_iter()
        .flat_map(|(_, entry)| entry.calls)
        .collect();
    let generations = generations
        .into_iter()
        .flat_map(|(_, entry)| entry.calls)
        .collect();
    Ok(Database {
        calls: pair_mirrors(steps, generations),
        models,
    })
}

fn read_metadata(connection: &Connection, query: &str, step: bool) -> Result<Vec<(i64, Metadata)>> {
    let mut statement = connection.prepare(query)?;
    let mut rows = statement.query([])?;
    let mut entries = Vec::new();
    while let Some(row) = rows.next()? {
        let data = row.get_ref(1)?;
        if step && data == rusqlite::types::ValueRef::Null {
            continue;
        }
        let idx = row.get(0)?;
        let entry = Metadata::decode(data.as_blob()?, step).with_context(|| {
            let table = if step { "steps" } else { "gen_metadata" };
            format!("decode {table} metadata at index {idx}")
        })?;
        entries.push((idx, entry));
    }
    entries.sort_by_key(|(idx, _)| *idx);
    Ok(entries)
}

fn fill_calls(
    entries: &mut [(i64, Metadata)],
    generation_models: &HashMap<i64, &Model>,
    fallback: (DateTime<Utc>, u8),
) {
    for (idx, entry) in entries {
        for call in &mut entry.calls {
            call.id = if call.id == 0 {
                entry.model.id
            } else {
                call.id
            };
            let matches =
                |model: &Model| (call.id == 0 || model.id == call.id) && !model.name.is_empty();
            let model = Some(&entry.model)
                .filter(|model| matches(model))
                .or_else(|| {
                    generation_models
                        .get(idx)
                        .copied()
                        .filter(|model| matches(model))
                });
            if let Some(model) = model {
                call.model.clone_from(&model.name);
                call.model_rank = if is_concrete_model(&call.model) { 4 } else { 1 };
            }
            (call.at, call.time_rank) = entry.timestamp.map_or(fallback, |at| (at, 2));
        }
    }
}

struct Metadata {
    model: Model,
    timestamp: Option<DateTime<Utc>>,
    calls: Vec<Call>,
}

struct Model {
    id: u64,
    name: String,
}

impl Metadata {
    fn decode(bytes: &[u8], step: bool) -> Result<Self> {
        let root = Fields::decode(bytes)?;
        let (data, step_model);
        let (source, model, usage_field, retry_field, id_field, name_fields, timestamp) = if step {
            step_model = root.nested(24)?;
            let timestamp = root.nested(8)?.timestamp().or(root.nested(1)?.timestamp());
            (&root, &step_model, 9, 28, 1, [12, 8], timestamp)
        } else {
            data = Fields::decode(root.bytes(1).context("missing generation metadata")?)?;
            let timestamp = data.nested(9)?.nested(4)?.timestamp();
            (&data, &data, 4, 17, 3, [19, 21], timestamp)
        };

        let primary = source.nested(usage_field)?;
        let main_call = Call::decode(&primary)?;
        let has_primary = main_call.is_some();
        let mut calls: Vec<_> = main_call.into_iter().collect();
        for retry in source.repeated_bytes(retry_field) {
            let retry = Fields::decode(retry?)?.nested(2)?;
            let Some(call) = Call::decode(&retry)? else {
                continue;
            };
            if let Some(main) = calls.first_mut().filter(|_| has_primary)
                && main.can_be_mirror(&call)
            {
                main.merge_mirror(call);
            } else {
                calls.push(call);
            }
        }
        let names = name_fields.map(|field| model.text(field).map(normalize_model));
        let [first, second] = names;
        let names = [first?, second?];
        let name = names
            .iter()
            .find(|name| is_concrete_model(name))
            .or_else(|| {
                names.iter().find(|name| {
                    !name.is_empty() && !name.starts_with("model_") && !name.starts_with("model-")
                })
            });
        let id = model.number(id_field);
        Ok(Self {
            model: Model {
                id: if id == 0 { primary.number(1) } else { id },
                name: name.cloned().unwrap_or_default(),
            },
            timestamp,
            calls,
        })
    }
}

#[derive(Clone, Copy, Default, Hash, PartialEq, Eq)]
struct Counters {
    uncached_input: u64,
    output: u64,
    cache_creation: u64,
    cache_read: u64,
    reasoning: u64,
    visible_output: u64,
}

impl Counters {
    fn total_output(self) -> u64 {
        self.output.max(self.reasoning + self.visible_output)
    }
}

#[derive(Clone)]
pub(super) struct Call {
    id: u64,
    ids: [String; 3],
    counters: Counters,
    pub(super) model: String,
    model_rank: u8,
    pub(super) at: DateTime<Utc>,
    time_rank: u8,
}

impl Call {
    fn decode(usage: &Fields<'_>) -> Result<Option<Self>> {
        let counters = Counters {
            uncached_input: usage.number(2),
            output: usage.number(3),
            cache_creation: usage.number(4),
            cache_read: usage.number(5),
            reasoning: usage.number(9),
            visible_output: usage.number(10),
        };
        if counters == Counters::default() {
            return Ok(None);
        }
        Ok(Some(Self {
            id: usage.number(1),
            ids: [
                usage.text(11)?.into(),
                usage.text(12)?.into(),
                usage.text(7)?.into(),
            ],
            counters,
            model: String::new(),
            model_rank: 0,
            at: DateTime::<Utc>::UNIX_EPOCH,
            time_rank: 0,
        }))
    }

    pub(super) fn tokens(&self) -> RequestTokens {
        let counts = self.counters;
        RequestTokens {
            input: counts.uncached_input + counts.cache_creation + counts.cache_read,
            cache_read: counts.cache_read,
            cache_creation: counts.cache_creation,
            output: counts.total_output(),
        }
    }

    fn keys(&self) -> impl Iterator<Item = (usize, &str)> {
        self.ids
            .iter()
            .enumerate()
            .filter(|(_, id)| !id.is_empty())
            .map(|(field, id)| (field, id.as_str()))
    }

    fn can_be_mirror(&self, other: &Self) -> bool {
        self.counters == other.counters
            && (self.id == 0 || other.id == 0 || self.id == other.id)
            && (self.keys().next().is_none() || other.keys().next().is_none())
    }

    fn merge_mirror(&mut self, other: Self) {
        if self.keys().next().is_none() {
            self.ids.clone_from(&other.ids);
        }
        if self.id == 0 {
            self.id = other.id;
        }
        self.merge_context(other);
    }

    fn merge_context(&mut self, other: Self) {
        if other.model_rank > self.model_rank {
            self.model = other.model;
            self.model_rank = other.model_rank;
        }
        if other.time_rank > self.time_rank
            || (other.time_rank == self.time_rank && other.at < self.at)
        {
            (self.at, self.time_rank) = (other.at, other.time_rank);
        }
    }

    fn merge(&mut self, other: Self) {
        // Merge the visible + reasoning breakdown as one value. Taking their
        // maxima independently could combine different copies' token splits.
        let counts = &mut self.counters;
        counts.uncached_input = counts.uncached_input.max(other.counters.uncached_input);
        counts.cache_creation = counts.cache_creation.max(other.counters.cache_creation);
        counts.cache_read = counts.cache_read.max(other.counters.cache_read);
        counts.output = counts.output.max(other.counters.total_output());
        self.merge_context(other);
    }

    pub(super) fn deduplicate(
        calls: Vec<(usize, Self)>,
        models: &HashMap<u64, Option<String>>,
    ) -> Vec<(usize, Self)> {
        let mut parent: Vec<usize> = (0..calls.len()).collect();
        let mut seen = HashMap::new();
        for (index, (_, call)) in calls.iter().enumerate() {
            for key in call.keys() {
                if let Some(old) = seen.insert(key, index) {
                    let a = root(&mut parent, old);
                    let b = root(&mut parent, index);
                    parent[a.max(b)] = a.min(b);
                }
            }
        }
        drop(seen);
        let mut groups: Vec<Option<(usize, Self)>> = vec![None; calls.len()];
        for (index, (source, mut call)) in calls.into_iter().enumerate() {
            call.resolve_model(models);
            let group = &mut groups[root(&mut parent, index)];
            match group {
                Some((_, first)) => first.merge(call),
                None => *group = Some((source, call)),
            }
        }
        groups.into_iter().flatten().collect()
    }

    fn resolve_model(&mut self, models: &HashMap<u64, Option<String>>) {
        if self.model_rank == 4 {
            return;
        }
        match models.get(&self.id) {
            Some(Some(name)) => {
                self.model.clone_from(name);
                self.model_rank = 3;
                return;
            }
            None => {
                if let Some(name) = model_by_id(self.id) {
                    self.model = name.into();
                    self.model_rank = 2;
                    return;
                }
            }
            Some(None) => {}
        }
        if self.model.is_empty() {
            self.model = if self.id == 0 {
                "antigravity-unknown".into()
            } else {
                format!("antigravity-model-{}", self.id)
            };
        }
    }
}

fn root(parent: &mut [usize], mut index: usize) -> usize {
    while parent[index] != index {
        parent[index] = parent[parent[index]];
        index = parent[index];
    }
    index
}

// Without response IDs, the same call often appears in both tables. Pair
// occurrences within this DB only, preserving repeated calls in either table.
fn pair_mirrors(mut steps: Vec<Call>, generations: Vec<Call>) -> Vec<Call> {
    let step_keys: HashSet<_> = steps.iter().flat_map(Call::keys).collect();
    let generation_keys: HashSet<_> = generations.iter().flat_map(Call::keys).collect();
    let generation_has_step: Vec<_> = generations
        .iter()
        .map(|call| call.keys().any(|key| step_keys.contains(&key)))
        .collect();
    let mut buckets: HashMap<Counters, VecDeque<usize>> = HashMap::new();
    for (index, step) in steps.iter().enumerate() {
        if !step.keys().any(|key| generation_keys.contains(&key)) {
            buckets.entry(step.counters).or_default().push_back(index);
        }
    }
    drop(step_keys);
    drop(generation_keys);
    for (generation, has_step) in generations.into_iter().zip(generation_has_step) {
        if !has_step
            && let Some(bucket) = buckets.get_mut(&generation.counters)
            && let Some(step) = bucket
                .iter()
                .position(|&index| steps[index].can_be_mirror(&generation))
                .and_then(|index| bucket.remove(index))
        {
            steps[step].merge_mirror(generation);
        } else {
            steps.push(generation);
        }
    }
    steps
}

pub(super) fn remember_model(
    models: &mut HashMap<u64, Option<String>>,
    id: u64,
    name: Option<&str>,
) {
    models
        .entry(id)
        .and_modify(|existing| {
            if existing.as_deref() != name {
                *existing = None;
            }
        })
        .or_insert_with(|| name.map(str::to_owned));
}

fn normalize_model(name: &str) -> String {
    let lower = name.to_lowercase().replace(['(', ')'], " ");
    let mut name = lower.split_whitespace().collect::<Vec<_>>().join("-");
    if let Some(claude) = name.strip_prefix("claude-") {
        if let Some((version, family)) = claude.split_once('-')
            && version.starts_with(|c: char| c.is_ascii_digit())
            && ["opus", "sonnet", "haiku"]
                .iter()
                .any(|prefix| family.starts_with(prefix))
        {
            let (family, suffix) = family.split_once('-').unwrap_or((family, ""));
            name = if suffix.is_empty() {
                format!("claude-{family}-{version}")
            } else {
                format!("claude-{family}-{version}-{suffix}")
            };
        }
        name = name.replace('.', "-");
    }
    name
}

fn is_concrete_model(name: &str) -> bool {
    if matches!(
        name,
        "gemini-3-flash-agent"
            | "gemini-3-flash-agent-a"
            | "gemini-3-flash-agent-b"
            | "gemini-3-flash-a"
            | "gemini-3-flash-b"
    ) {
        return false;
    }
    name.starts_with("gpt-")
        || ["gemini-", "claude-opus-", "claude-sonnet-", "claude-haiku-"]
            .iter()
            .any(|prefix| {
                name.strip_prefix(prefix)
                    .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
            })
}

fn model_by_id(id: u64) -> Option<&'static str> {
    // Offline IDs from T3 Code. Concrete names found in conversation metadata
    // take precedence, including names absent from this table.
    Some(match id {
        246 => "gemini-2.5-pro",
        312 => "gemini-2.5-flash",
        313 | 329 => "gemini-2.5-flash-thinking",
        330 => "gemini-2.5-flash-lite",
        281 | 282 => "claude-sonnet-4",
        290 | 291 => "claude-opus-4",
        333 | 334 => "claude-sonnet-4-5",
        340 | 341 => "claude-haiku-4-5",
        342 => "gpt-oss-120b-medium",
        1026 => "claude-opus-4-6",
        1035 => "claude-sonnet-4-6",
        1016 | 1036 | 1037 => "gemini-3.1-pro",
        1018 | 1047 | 1084 => "gemini-3-flash-preview",
        1020 => "gemini-3.5-flash-medium",
        1021 => "gemini-3.1-flash-image",
        1050 => "gemini-3.1-flash-lite",
        1071 => "gemini-3.6-flash-high",
        1072 => "gemini-3.6-flash-medium",
        1073 => "gemini-3.6-flash-low",
        1132 | 1133 => "gemini-3.5-flash-high",
        1187 => "gemini-3.5-flash-extra-low",
        1196 | 1266 => "gemini-3.6-flash",
        1298 => "gemini-3.7-flash-high",
        1299 => "gemini-3.7-flash-medium",
        1300 => "gemini-3.7-flash-low",
        1301 => "gemini-3.7-flash-tiered",
        1318 => "gemini-3.8-flash-high",
        1319 => "gemini-3.8-flash-medium",
        1320 => "gemini-3.8-flash-low",
        1322 => "gemini-3.8-flash-tiered",
        1402 => "claude-opus-5-5",
        1417..=1419 => "gemini-4-argon",
        1537 => "gemini-pro-orchestrator-neon",
        _ => return None,
    })
}

pub(super) fn rate_model(model: &str) -> &str {
    match model {
        "gemini-3-flash-agent"
        | "gemini-3-flash-agent-a"
        | "gemini-3-flash-agent-b"
        | "gemini-3-flash-a"
        | "gemini-3-flash-b" => return "gemini-3.5-flash",
        "gemini-3.7-flash-safety-le" => return "gemini-3.7-flash",
        "gemini-pro-orchestrator-neon" => return "gemini-4-neon",
        "gemini-pro-orchestrator-g1-pro" => return "gemini-4-argon",
        _ => {}
    }
    let base = [
        "-extra-low",
        "-low",
        "-medium",
        "-high",
        "-tiered",
        "-control",
        "-thinking",
        "-agent",
        "-orchestrator",
    ]
    .iter()
    .find_map(|suffix| model.strip_suffix(suffix))
    .unwrap_or(model);
    if base == "gemini-3-flash" {
        "gemini-3-flash-preview"
    } else {
        base
    }
}

#[derive(Clone, Copy)]
enum Field<'a> {
    Number(u64),
    Bytes(&'a [u8]),
}

#[derive(Default)]
struct Fields<'a>(Vec<(u32, Field<'a>)>);

impl<'a> Fields<'a> {
    fn decode(mut bytes: &'a [u8]) -> Result<Self> {
        let mut fields = Vec::new();
        while !bytes.is_empty() {
            let tag = u32::try_from(varint(&mut bytes)?)?;
            let number = tag >> 3;
            if number == 0 {
                bail!("invalid protobuf field number");
            }
            let field = match tag & 7 {
                0 => Field::Number(varint(&mut bytes)?),
                2 => {
                    let length = usize::try_from(varint(&mut bytes)?)?;
                    Field::Bytes(take(&mut bytes, length)?)
                }
                wire @ (1 | 5) => {
                    take(&mut bytes, if wire == 1 { 8 } else { 4 })?;
                    continue;
                }
                _ => bail!("unsupported protobuf wire type"),
            };
            fields.push((number, field));
        }
        Ok(Self(fields))
    }

    fn first(&self, key: u32) -> Option<Field<'a>> {
        self.0
            .iter()
            .find(|(number, _)| *number == key)
            .map(|(_, field)| *field)
    }

    fn number(&self, key: u32) -> u64 {
        // T3 ignores values outside the safe integer range (including encoded
        // negative counters). Those values are never valid token counts.
        match self.first(key) {
            Some(Field::Number(n)) if n < (1 << 53) => n,
            _ => 0,
        }
    }

    fn bytes(&self, key: u32) -> Option<&'a [u8]> {
        match self.first(key) {
            Some(Field::Bytes(bytes)) => Some(bytes),
            _ => None,
        }
    }

    fn repeated_bytes(&self, key: u32) -> impl Iterator<Item = Result<&'a [u8]>> {
        self.0
            .iter()
            .filter(move |(number, _)| *number == key)
            .map(|(_, field)| match field {
                Field::Bytes(bytes) => Ok(*bytes),
                Field::Number(_) => bail!("expected protobuf message"),
            })
    }

    fn nested(&self, key: u32) -> Result<Self> {
        self.bytes(key)
            .map_or_else(|| Ok(Self::default()), Self::decode)
    }

    fn text(&self, key: u32) -> Result<&'a str> {
        Ok(str::from_utf8(self.bytes(key).unwrap_or_default())?.trim())
    }

    fn timestamp(&self) -> Option<DateTime<Utc>> {
        let seconds = self.number(1);
        if seconds == 0 {
            return None;
        }
        DateTime::from_timestamp(seconds as i64, u32::try_from(self.number(2)).ok()?)
    }
}

fn varint(bytes: &mut &[u8]) -> Result<u64> {
    let mut value = 0;
    for shift in (0..=63).step_by(7) {
        let byte = take(bytes, 1)?[0];
        if shift == 63 && byte > 1 {
            break;
        }
        value |= u64::from(byte & 127) << shift;
        if byte < 128 {
            return Ok(value);
        }
    }
    bail!("invalid protobuf varint")
}

fn take<'a>(bytes: &mut &'a [u8], length: usize) -> Result<&'a [u8]> {
    let (value, rest) = bytes
        .split_at_checked(length)
        .context("truncated protobuf field")?;
    *bytes = rest;
    Ok(value)
}
