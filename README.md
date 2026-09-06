# kanban-stale-cards

Boards accumulate cards nobody looks at. A card slides into "In Review" and
sits there for three weeks because everyone's attention moved on. Trello
(and most other kanban tools) will happily export the whole board as JSON,
but nothing in the export itself tells you which cards have gone stale.

This is a small command-line tool that answers exactly that question: for
every open card, how long has it been sitting in the list it's currently in,
sorted with the stalest card first.

## Getting a board export

In Trello: open the board, click **Show Menu > More > Print, export, and
share > Export as JSON**. If you export through the Trello API (or a
Power-Up) and include the `actions` field, the tool can tell precisely when
a card entered its current list. Without `actions` it falls back to the
card's `dateLastActivity`, which is a reasonable approximation but will also
move if someone edits the description or adds a comment without changing
lists.

## Usage

```
cargo build --release
./target/release/kanban-stale-cards board-export.json
```

Human-readable output:

```
CARD                          LIST         STALE
Migrate billing to Stripe     In Review    14d 6h
Fix flaky checkout test       In Progress  6d 2h
Write onboarding docs         To Do        1d 0h
```

Machine-readable output with `--json`:

```
$ kanban-stale-cards board-export.json --json
[
  {"card": "Migrate billing to Stripe", "list": "In Review", "entered_at_unix": 1717000000, "stale_seconds": 1234000, "stale_human": "14d 6h"},
  {"card": "Fix flaky checkout test", "list": "In Progress", "entered_at_unix": 1718100000, "stale_seconds": 530000, "stale_human": "6d 2h"}
]
```

Only show the five stalest cards:

```
kanban-stale-cards board-export.json --limit 5
```

## How it works

There are no dependencies, so the tool includes its own minimal JSON reader
(`src/json.rs`) and its own ISO 8601 UTC timestamp parser (`src/time.rs`,
using the days-from-civil calendar algorithm rather than pulling in a date
library). `src/board.rs` walks the parsed export: it builds a map of each
card's current list, then scans the `actions` array for the most recent
`updateCard` action that moved the card into that list. If there's no
`actions` array in the export, it uses `dateLastActivity` instead.

Archived (closed) cards and archived lists are skipped, since a card nobody
can see anymore isn't something you can act on.

## License

MIT, see [LICENSE](LICENSE).
