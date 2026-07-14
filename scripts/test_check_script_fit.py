#!/usr/bin/env python3

"""Focused request-shape tests for the brokered Deepgram fallback."""

import importlib.util
import os
from pathlib import Path
import unittest
from unittest.mock import patch
import urllib.error


MODULE_PATH = Path(__file__).with_name("check_script_fit.py")
SPEC = importlib.util.spec_from_file_location("check_script_fit", MODULE_PATH)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class DeepgramMintRequestTest(unittest.TestCase):
    def test_base_url_is_derived_from_mint_and_exact_config_is_accepted(self) -> None:
        with patch.dict(
            os.environ,
            {
                "MINT_BASE_URL": "http://mint:4949/",
                "DEEPGRAM_BASE_URL": "http://mint:4949/proxy/https/api.deepgram.com/v1/",
            },
            clear=True,
        ):
            self.assertEqual(
                MODULE.deepgram_base_url(),
                "http://mint:4949/proxy/https/api.deepgram.com/v1",
            )

    def test_vendor_or_inexact_origins_are_rejected(self) -> None:
        invalid_environments = [
            {
                "MINT_BASE_URL": "https://api.deepgram.com",
                "DEEPGRAM_BASE_URL": "https://api.deepgram.com/v1",
            },
            {
                "MINT_BASE_URL": "http://mint:4949",
                "DEEPGRAM_BASE_URL": "https://api.deepgram.com/v1",
            },
            {
                "MINT_BASE_URL": "http://mint:4949",
                "DEEPGRAM_BASE_URL": "http://mint:4949/proxy/https/api.deepgram.com/v2",
            },
            {
                "DEEPGRAM_BASE_URL": "http://mint:4949/proxy/https/api.deepgram.com/v1",
            },
        ]
        for environment in invalid_environments:
            with self.subTest(environment=environment), patch.dict(
                os.environ, environment, clear=True
            ):
                with self.assertRaises(RuntimeError):
                    MODULE.deepgram_base_url()

    def test_request_uses_mint_url_and_placeholder(self) -> None:
        observed = {}

        class FakeResponse:
            def __enter__(self):
                return self

            def __exit__(self, *_args):
                return False

            def read(self):
                return b'{"results":{"channels":[{"alternatives":[{"transcript":"ok","words":[]}]}]}}'

        def fake_urlopen(request):
            observed["url"] = request.full_url
            observed["authorization"] = request.headers["Authorization"]
            return FakeResponse()

        completed = type("Completed", (), {"stdout": b"wav"})()
        with patch.dict(
            os.environ, {"MINT_BASE_URL": "http://mint"}, clear=True
        ), patch.object(MODULE.subprocess, "run", return_value=completed), patch.object(
            MODULE.urllib.request, "urlopen", side_effect=fake_urlopen
        ):
            MODULE.transcribe_deepgram(
                Path("fixture.mp4"),
                "http://mint/proxy/https/api.deepgram.com/v1",
            )

        self.assertEqual(
            observed["url"],
            "http://mint/proxy/https/api.deepgram.com/v1/listen?model=nova-3&language=en&punctuate=true",
        )
        self.assertEqual(observed["authorization"], "Token __mint.deepgram.default__")

    def test_direct_vendor_request_is_rejected_before_ffmpeg(self) -> None:
        with patch.dict(
            os.environ, {"MINT_BASE_URL": "http://mint:4949"}, clear=True
        ), patch.object(MODULE.subprocess, "run") as ffmpeg:
            with self.assertRaises(RuntimeError):
                MODULE.transcribe_deepgram(
                    Path("fixture.mp4"),
                    "https://api.deepgram.com/v1",
                )
        ffmpeg.assert_not_called()

    def test_healthy_fal_runs_first_even_when_deepgram_is_configured(self) -> None:
        fal_result = {"text": "fal", "chunks": []}
        environment = {
            "FAL_API_KEY": "fal-placeholder-for-test",
            "MINT_BASE_URL": "http://mint:4949",
            "DEEPGRAM_BASE_URL": "http://mint:4949/proxy/https/api.deepgram.com/v1",
        }
        with patch.dict(os.environ, environment, clear=True), patch.object(
            MODULE, "transcribe", return_value=fal_result
        ) as fal, patch.object(MODULE, "transcribe_deepgram") as deepgram:
            source, result = MODULE.transcribe_any(Path("fixture.mp4"), "word")

        self.assertEqual((source, result), ("fal_whisper", fal_result))
        fal.assert_called_once_with(Path("fixture.mp4"), "fal-placeholder-for-test", "word")
        deepgram.assert_not_called()

    def test_missing_fal_falls_back_to_deepgram(self) -> None:
        deepgram_result = {"text": "deepgram", "chunks": []}
        environment = {"MINT_BASE_URL": "http://mint:4949"}
        with patch.dict(os.environ, environment, clear=True), patch.object(
            MODULE, "transcribe"
        ) as fal, patch.object(
            MODULE, "transcribe_deepgram", return_value=deepgram_result
        ) as deepgram:
            source, result = MODULE.transcribe_any(Path("fixture.mp4"), "segment")

        self.assertEqual((source, result), ("deepgram", deepgram_result))
        fal.assert_not_called()
        deepgram.assert_called_once_with(
            Path("fixture.mp4"),
            "http://mint:4949/proxy/https/api.deepgram.com/v1",
        )

    def test_failed_fal_falls_back_after_the_failure(self) -> None:
        calls = []
        deepgram_result = {"text": "deepgram", "chunks": []}
        environment = {
            "FAL_API_KEY": "fal-placeholder-for-test",
            "MINT_BASE_URL": "http://mint:4949",
        }

        def fail_fal(*_args):
            calls.append("fal")
            raise urllib.error.HTTPError("https://queue.fal.run", 503, "down", {}, None)

        def use_deepgram(*_args):
            calls.append("deepgram")
            return deepgram_result

        with patch.dict(os.environ, environment, clear=True), patch.object(
            MODULE, "transcribe", side_effect=fail_fal
        ), patch.object(MODULE, "transcribe_deepgram", side_effect=use_deepgram):
            source, result = MODULE.transcribe_any(Path("fixture.mp4"), "segment")

        self.assertEqual(calls, ["fal", "deepgram"])
        self.assertEqual((source, result), ("deepgram", deepgram_result))

    def test_invalid_deepgram_config_does_not_preempt_healthy_fal(self) -> None:
        fal_result = {"text": "fal", "chunks": []}
        environment = {
            "FAL_API_KEY": "fal-placeholder-for-test",
            "MINT_BASE_URL": "http://mint:4949",
            "DEEPGRAM_BASE_URL": "https://api.deepgram.com/v1",
        }
        with patch.dict(os.environ, environment, clear=True), patch.object(
            MODULE, "transcribe", return_value=fal_result
        ), patch.object(MODULE, "transcribe_deepgram") as deepgram:
            source, result = MODULE.transcribe_any(Path("fixture.mp4"), "segment")

        self.assertEqual((source, result), ("fal_whisper", fal_result))
        deepgram.assert_not_called()


if __name__ == "__main__":
    unittest.main()
