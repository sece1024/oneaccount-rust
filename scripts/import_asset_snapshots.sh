#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <csv_file> [api_base]"
  echo "Example: $0 data/money\ total\ -\ asset\ table.csv http://127.0.0.1:8080/api/v1"
  exit 1
fi

CSV_FILE="$1"
API_BASE="${2:-http://127.0.0.1:8080/api/v1}"

if [[ ! -f "$CSV_FILE" ]]; then
  echo "CSV file not found: $CSV_FILE"
  exit 1
fi

if ! curl -fsS "$API_BASE/accounts" >/dev/null; then
  echo "API not reachable: $API_BASE"
  echo "Start backend first, e.g. make run-server or make dev"
  exit 1
fi

ensure_summary_account() {
  local currency="$1"
  local name="CSV资产汇总(${currency})"
  local accounts_json
  local account_id

  accounts_json="$(curl -fsS "$API_BASE/accounts" | tr -d '\n')"
  account_id="$({
    printf '%s' "$accounts_json" | awk -v target="\"name\":\"${name}\"" -v curr="\"currency\":\"${currency}\"" '
      BEGIN { RS = "\\},\\{" }
      {
        if (index($0, target) && index($0, curr) && match($0, /"id":[0-9]+/)) {
          print substr($0, RSTART + 5, RLENGTH - 5)
          exit
        }
      }
    '
  })"

  if [[ -z "$account_id" ]]; then
    local payload
    local resp
    payload="{\"name\":\"${name}\",\"account_type\":\"bank\",\"currency\":\"${currency}\",\"initial_balance\":0,\"is_liquid\":true}"
    resp="$(curl -fsS -X POST "$API_BASE/accounts" -H 'Content-Type: application/json' -d "$payload")"
    account_id="$(printf '%s' "$resp" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')"
    if [[ -z "$account_id" ]]; then
      echo "Failed to create account for $currency"
      echo "$resp"
      exit 1
    fi
    echo "Created account $name (id=$account_id)" >&2
  else
    echo "Reusing account $name (id=$account_id)" >&2
  fi

  printf '%s' "$account_id"
}

import_currency_snapshots() {
  local currency="$1"
  local account_id="$2"
  local tmp_file
  local imported=0
  local failed=0

  tmp_file="$(mktemp)"

  awk -F',' -v currency="$currency" '
    NR > 1 && $4 == currency {
      split($5, a, "/")
      if (a[1] && a[2] && a[3]) {
        key = sprintf("%04d-%02d-%02d", a[3], a[1], a[2])
        day_sum[key] += $3 + 0
      }
    }
    END {
      for (d in day_sum) print d "," day_sum[d]
    }
  ' "$CSV_FILE" | sort | awk -F',' '
    {
      month = substr($1, 1, 7)
      month_balance[month] = $2
    }
    END {
      for (m in month_balance) print m "," month_balance[m]
    }
  ' | sort > "$tmp_file"

  if [[ ! -s "$tmp_file" ]]; then
    rm -f "$tmp_file"
    echo "No rows for currency=$currency"
    return 0
  fi

  while IFS=',' read -r ym balance; do
    local year="${ym%-*}"
    local month="${ym#*-}"
    local month_num=$((10#$month))
    local payload
    local resp

    payload="{\"year\":${year},\"month\":${month_num},\"balances\":[{\"account_id\":${account_id},\"balance\":${balance}}],\"note\":\"imported from ${CSV_FILE} (${currency} monthly final)\"}"
    resp="$(curl -sS -X POST "$API_BASE/snapshots" -H 'Content-Type: application/json' -d "$payload")"

    if printf '%s' "$resp" | grep -q '"total"'; then
      imported=$((imported + 1))
    else
      failed=$((failed + 1))
      echo "Failed month $ym ($currency): $resp"
    fi
  done < "$tmp_file"

  echo "Imported $imported monthly snapshots for $currency (failed=$failed)"
  echo "Sample rows for $currency:"
  head -n 2 "$tmp_file" || true
  tail -n 2 "$tmp_file" || true

  rm -f "$tmp_file"
}

if [[ "${SKIP_BACKUP:-0}" != "1" ]]; then
  echo "Creating DB backup before import..."
  cargo run -- backup >/dev/null
fi

for currency in CNY HKD; do
  account_id="$(ensure_summary_account "$currency")"
  import_currency_snapshots "$currency" "$account_id"
done

echo "Done. Current summary accounts:"
curl -fsS "$API_BASE/accounts" | tr -d '\n' | sed 's/},{/},\n{/g' | grep 'CSV资产汇总' || true
