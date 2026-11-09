"""Generate spending/savings summaries for cockpit dashboard."""
from collections import defaultdict
from datetime import datetime
from .parser import Transaction


def generate_summary(transactions: list[Transaction]) -> dict:
    """Generate spending summary suitable for cockpit charts."""
    by_category: dict[str, float] = defaultdict(float)
    by_month: dict[str, float] = defaultdict(float)
    total_income = 0.0
    total_expense = 0.0
    recurring_total = 0.0

    for txn in transactions:
        month_key = txn.date.strftime("%Y-%m")
        if txn.amount > 0:
            total_income += txn.amount
        else:
            total_expense += abs(txn.amount)
            cat = txn.category or "outros"
            by_category[cat] += abs(txn.amount)
            by_month[month_key] += abs(txn.amount)
        if txn.is_recurring:
            recurring_total += abs(txn.amount)

    sorted_categories = sorted(by_category.items(), key=lambda x: x[1], reverse=True)
    sorted_months = sorted(by_month.items())

    return {
        "total_income": round(total_income, 2),
        "total_expense": round(total_expense, 2),
        "net_balance": round(total_income - total_expense, 2),
        "recurring_expense": round(recurring_total, 2),
        "transaction_count": len(transactions),
        "by_category": [{"category": c, "amount": round(a, 2)} for c, a in sorted_categories],
        "by_month": [{"month": m, "amount": round(a, 2)} for m, a in sorted_months],
        "generated_at": datetime.now().isoformat(),
    }
