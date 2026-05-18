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
  echo "CSV file not found: $CSV_FILE" >&2
  exit 1
fi

# Check if jq is available
if ! command -v jq &> /dev/null; then
  echo "jq is required but not installed. Install with: brew install jq (macOS) or apt-get install jq (Linux)" >&2
  exit 1
fi

if ! curl -fsS "$API_BASE/accounts" >/dev/null 2>&1; then
  echo "API not reachable: $API_BASE" >&2
  echo "Start backend first, e.g. make run-server or make dev" >&2
  exit 1
fi

# Safely extract account ID by currency and name using jq
find_account_by_currency() {
  local currency="$1"
  local name="$2"
  
  local accounts_json
  accounts_json="$(curl -fsS "$API_BASE/accounts" 2>/dev/null)"
  
  # Use jq to find matching account safely
  printf '%s' "$accounts_json" | jq -r ".[] | select(.currency == \"$currency\" and .name == \"$name\") | .id" | head -1
}

# Create account with jq-built JSON payload
create_account() {
  local currency="$1"
  local name="$2"
  
  local payload
  payload=$(jq -n \
    --arg name "$name" \
    --arg account_type "bank" \
    --arg currency "$currency" \
    --argjson initial_balance 0 \
    --argjson is_liquid true \
    '{name: $name, account_type: $account_type, currency: $currency, initial_balance: $initial_balance, is_liquid: $is_liquid}')
  
  local resp
  resp="$(curl -fsS -X POST "$API_BASE/accounts" \
    -H 'Content-Type: application/json' \
    -d "$payload" 2>/dev/null)"
  
  printf '%s' "$resp" | jq -r '.id' 2>/dev/null || echo ""
}

ensure_summary_account() {
  local currency="$1"
  local name="CSV资产汇总(${currency})"
  local account_id

  account_id="$(find_account_by_currency "$currency" "$name")"

  if [[ -z "$account_id" || "$account_id" == "null" ]]; then
    account_id="$(create_account "$currency" "$name")"
    if [[ -z "$account_id" || "$account_id" == "null" ]]; then
      echo "Failed to create account for $currency" >&2
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

  # Extract monthly balances for the currency
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
    echo "No rows for currency=$currency" >&2
    return 0
  fi

  while IFS=',' read -r ym balance; do
    local year="${ym%-*}"
    local month="${ym#*-}"
    local month_num=$((10#$month))
    local payload
    local resp

    # Build JSON payload safely with jq (numbers are properly typed)
    payload=$(jq -n \
      --argjson year "$year" \
      --argjson month "$month_num" \
      --argjson account_id "$account_id" \
      --argjson balance "$balance" \
      --arg csv_file "$CSV_FILE" \
      --arg currency "$currency" \
      '{year: $year, month: $month, balances: [{account_id: $account_id, balance: $balance}], note: ("imported from \($csv_file) (\($currency) monthly final)")}')
    
    resp="$(curl -sS -X POST "$API_BASE/snapshots" \
      -H 'Content-Type: application/json' \
      -d "$payload" 2>/dev/null)"

    if printf '%s' "$resp" | jq -e '.total' >/dev/null 2>&1; then
      imported=$((imported + 1))
    else
      failed=$((failed + 1))
      echo "Failed month $ym ($currency): $resp" >&2
    fi
  done < "$tmp_file"

  echo "Imported $imported monthly snapshots for $currency (failed=$failed)" >&2
  echo "Sample rows for $currency:" >&2
  head -n 2 "$tmp_file" >&2 || true
  tail -n 2 "$tmp_file" >&2 || true

  rm -f "$tmp_file"
}

if [[ "${SKIP_BACKUP:-0}" != "1" ]]; then
  echo "Creating DB backup before import..." >&2
  cargo run -- backup >/dev/null 2>&1 || true
fi

for currency in CNY HKD; do
  account_id="$(ensure_summary_account "$currency")"
  import_currency_snapshots "$currency" "$account_id"
done

echo "Done. Current summary accounts:" >&2
curl -fsS "$API_BASE/accounts" 2>/dev/null | jq '.[] | select(.name | startswith("CSV资产汇总")) | {id, name, currency, balance}' || true
