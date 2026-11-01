"""Small bounded TypeSafe client for the shadow pilot, without automatic retries."""

import json
import math
import os
import subprocess
import time

ENDPOINT = "https://api.typesafe.ai/v1/systemone"


def probability(value):
    if type(value) not in (int, float) or not 0 <= value <= 1 or not math.isfinite(value):
        raise ValueError("invalid probability")
    return value


def validate(payload, response):
    """Validate and project onto known fields. Never retain arbitrary server text."""
    if response["model"] != payload["model"]:
        raise ValueError("unexpected model")
    if set(response["answers"]) != set(payload["questions"]):
        raise ValueError("unexpected answer keys")
    answers = {}
    for key, question in payload["questions"].items():
        answer = response["answers"][key]
        if answer["type"] != "choice" or question["type"] != "choice":
            raise ValueError("unexpected answer type")
        probabilities = answer["probabilities"]
        if set(probabilities) != set(question["criteria"]):
            raise ValueError("unexpected options")
        probabilities = {option: probability(value) for option, value in probabilities.items()}
        if abs(sum(probabilities.values()) - 1) > 0.03:
            raise ValueError("probabilities do not sum to one")
        choice = answer["choice"]
        if choice not in probabilities or probabilities[choice] < max(probabilities.values()):
            raise ValueError("invalid selection")
        answers[key] = {"type": "choice", "choice": choice,
                        "confidence": probability(answer["confidence"]), "probabilities": probabilities}
    usage = {}
    for name in ("input_tokens", "output_tokens"):
        value = response["usage"][name]
        if type(value) is not int or not 0 <= value <= 64000:
            raise ValueError("invalid usage")
        usage[name] = value
    return {"model": payload["model"], "answers": answers, "usage": usage}


def evaluate(payload, key):
    if not key or any(ord(char) < 33 or ord(char) > 126 for char in key):
        return {"status": "unavailable", "reason": "missing_or_invalid_credential", "elapsed_seconds": 0}
    started = time.monotonic()
    def quoted(value):
        return value.replace('\\', '\\\\').replace('"', '\\"')

    # Configuration on stdin keeps both the key and the excerpt off process argv.
    # No redirect flag is used. -q prevents user curl configuration changing this.
    config = 'header = "Authorization: Bearer ' + quoted(key) + '"\n'
    body = json.dumps(payload, ensure_ascii=True, sort_keys=True, separators=(',', ':'))
    config += 'data-binary = "' + quoted(body) + '"\n'
    environment = dict(os.environ)
    environment.pop('TYPESAFE_API_KEY', None)
    command = ['curl', '-q', '--silent', '--show-error', '--connect-timeout', '10',
               '--max-time', '30', '--max-filesize', '65536', '--config', '-',
               '--request', 'POST', '--header', 'Content-Type: application/json',
               '--write-out', '\nHTTP_STATUS=%{http_code}\n', ENDPOINT]
    result = {"status": "unavailable"}
    try:
        completed = subprocess.run(command, input=config, capture_output=True, text=True,
                                   timeout=35, env=environment, check=False)
        body, marker, status = completed.stdout.rpartition('\nHTTP_STATUS=')
        if not marker or not status.strip().isdigit():
            raise ValueError('missing response status')
        result['http_status'] = int(status.strip())
        if completed.returncode:
            raise OSError('transport failure')
        if result['http_status'] != 200:
            result.update(reason='service_error', elapsed_seconds=time.monotonic() - started)
            return result
        if len(body.encode()) > 65536:
            raise ValueError('oversized response')
        response = validate(payload, json.loads(body))
        result.update(status="ok", response=response)
    except (subprocess.TimeoutExpired, OSError):
        result["reason"] = "transport_error"
    except (ValueError, KeyError, TypeError, AttributeError, UnicodeError):
        result["reason"] = "invalid_response"
    result["elapsed_seconds"] = time.monotonic() - started
    return result
