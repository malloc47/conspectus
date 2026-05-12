# Conspectus Naming Notes

This is an archeological record of names considered for the standalone session,
workspace, mux, fork, and forge graph tool now called `conspectus`.

## Selected Name

`conspectus`

Meaning: a survey, synopsis, or comprehensive view. It fits the desired product
shape: a cross-tool status surface that assembles sparse evidence about agent
sessions, mux sessions, repos, worktrees, workspaces, forks, branches, and PRs.

Collision read: used as a general English and academic/business term, but no
obvious AI developer-tooling project collision was found in the initial search.

## Literal Names Considered

- `workgraph`: rejected because `graphwork/workgraph`, `aiida-workgraph`, Go
  packages named `workgraph`, and other WorkGraph-related projects already use
  the term.
- `session-graph`: plausible, direct, and not obviously occupied by a strong
  AI tooling project, but generic.
- `session-map`: plausible and friendlier than `session-graph`.
- `context-graph`: plausible, but close to a broad class of context graph and
  knowledge graph products.
- `context-map`: plausible, but generic.
- `session-mesh`: plausible but weak; "mesh" implies networking or distributed
  coordination more than local status and attribution.
- `agent-graph`: rejected because `agentgraph` exists as an AI/LLM task graph
  library and the name sits in a crowded agent framework space.
- `agent-mesh`: rejected because AgentMesh-style names are crowded in AI agent
  governance, registry, and coordination tooling.

## Deck Family

- `ctxdeck`
- `contextdeck`
- `forkdeck`
- `sessiondeck`

These had acceptable collision reads in the first pass, but were set aside
because the "deck" metaphor is too close to entrenched `agent-deck` usage for
this project.

## Manager And Registry Family

These were rejected or deprioritized because they imply orchestration/control
rather than discovery, mapping, and provenance:

- `agent-manager`
- `agents-manager`
- `agent-coordinator`
- `agent-registry`
- `agent-index`
- `agent-catalog`
- `agent-ledger`
- `steward`
- `custodian`
- `registrar`

The agent-prefixed variants also collide with an increasingly crowded AI agent
management and registry namespace.

## Higher-Register Names Considered

- `itinerarium`: strong candidate. An ancient or medieval route guide, often a
  list of paths and stops. Good semantic fit for sessions moving through
  contexts. No obvious AI tooling collision was found, but the word is long.
- `cartulary`: strong candidate. A register of charters or records. Good fit
  for durable declared links and provenance. More archival than navigational.
- `conspectus`: selected. A comprehensive view or survey. Best balance of
  elevated register, broad product fit, and low AI-tooling collision risk.
- `gazetteer`: plausible. A place-name index or catalog. Good for locating
  sessions and work contexts, but strongly associated with geospatial indexes.
- `cicerone`: plausible. A guide or docent. More guide-like than graph-like.
- `apparatus`: plausible. Evokes a scholarly critical apparatus: references,
  variants, provenance. Broad and somewhat heavy.
- `stemma`: rejected despite strong lineage meaning because Nage AI uses STEMMA
  as part of an AI architecture vocabulary.
- `periplus`: rejected despite strong navigation-guide meaning because it is
  already used near AI/market-agent tooling.
- `anabasis`: rejected because an AI career/hiring platform uses the name.
- `enchiridion`: rejected because Enchiridion Labs is explicitly AI-agent
  infrastructure.
- `concordance`: rejected because Concordance is an AI monitoring company.
- `scry`: rejected because it is already used by AI search/research products.
- `catena`: rejected because Catena Labs operates in AI-agent financial
  infrastructure.
- `loci`: rejected because LOCI and Locus are already used in AI coding and
  agent-memory tooling.
- `nexus`: rejected because Nexus is heavily occupied in AI agent platforms and
  agent registries.
- `synapse`: rejected because Synapse is heavily occupied in AI orchestration
  and agent communication tooling.

## Naming Direction

Prefer names that imply:

- survey over orchestration
- context and lineage over agent management
- discovery and provenance over control
- a durable working record over a workflow engine

Avoid names that sound like:

- AI agent frameworks
- multi-agent orchestrators
- agent registries
- generic graph databases
- workflow engines
