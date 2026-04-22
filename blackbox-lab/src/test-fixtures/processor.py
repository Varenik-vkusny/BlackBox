import json
import psycopg2
from typing import Dict, List

def process_batch(items: List[Dict]) -> Dict:
    """Process a batch of items from the queue."""
    connection = None
    try:
        # Line 9: Connect to PostgreSQL
        connection = psycopg2.connect(
            host="localhost",
            port=5432,
            user="app_user",
            password="secret",
            database="events_db"
        )

        cursor = connection.cursor()

        for item in items:
            # Line 19: Execute query
            cursor.execute(
                "INSERT INTO events (data, timestamp) VALUES (%s, NOW())",
                (json.dumps(item),)
            )

        connection.commit()
        return {"status": "success", "count": len(items)}

    except psycopg2.OperationalError as e:
        # Line 29: This error happens when PostgreSQL is down
        raise ConnectionError(f"PostgreSQL connection refused: {e}")
    finally:
        if connection:
            connection.close()

def validate_input(data: str) -> bool:
    """Validate input JSON."""
    try:
        json.loads(data)
        return True
    except json.JSONDecodeError as e:
        raise ValueError(f"Invalid JSON: {e}")
