"""Boundary regressions for the independent UHP request consumer."""

import json
import re
import unittest

from consumer import PACKAGE, valid_global_request


class TransposePaneIdTests(unittest.TestCase):
    """Keep schema and consumer pane-string ranges aligned without bigint parsing."""

    def test_string_range_matches_schema(self):
        """Reject overflow and 5,000-digit IDs while preserving leading zeros."""
        schema = json.loads((PACKAGE / "schema/request.schema.json").read_text())
        methods = set(schema["properties"]["method"]["enum"])
        pattern = schema["$defs"]["paneTransposeParams"]["properties"]["pane"]["oneOf"][0]["pattern"]
        cases = [(str(value), 0 <= value <= 4294967295) for value in (
            0, 7, 999999999, 1000000000, 4000000000, 4200000000,
            4290000000, 4294000000, 4294900000, 4294960000,
            4294967000, 4294967200, 4294967290, 4294967295, 4294967296,
            4294967300, 4294968000, 4294970000, 4295000000,
            4300000000, 5000000000, 9999999999,
        )]
        cases += [("9" * 5000, False), ("0" * 5000 + "7", True),
                  ("0004294967295", True), ("0004294967296", False),
                  ("", False), ("-1", False), ("１２", False)]
        for target, expected in cases:
            with self.subTest(target=target[:30], length=len(target)):
                request = {"id": "transpose", "method": "pane.transpose",
                           "params": {"pane": target}}
                self.assertEqual(valid_global_request(request, methods), expected)
                self.assertEqual(re.fullmatch(pattern, target) is not None, expected)


class PaneFloatTests(unittest.TestCase):
    """Keep pane.float fixtures and the consumer contract in step."""

    def setUp(self):
        schema = json.loads((PACKAGE / "schema/request.schema.json").read_text())
        self.methods = set(schema["properties"]["method"]["enum"])
        self.params = schema["$defs"]["paneFloatParams"]

    def assertValid(self, params, expected):
        request = {"id": "float", "method": "pane.float", "params": params}
        self.assertEqual(valid_global_request(request, self.methods), expected)

    def test_geometry_and_bounded_fields(self):
        self.assertValid({"pane": "7", "width": 40, "height": 12,
                          "x": 4, "y": 2, "title": "Review",
                          "focus": False, "command": ["htop"]}, True)
        self.assertValid({}, True)
        self.assertValid({"pane": 7, "width": 3, "height": 3, "x": 0, "y": 0}, True)
        for key in ("width", "height"):
            minimum = self.params["properties"][key]["minimum"]
            self.assertValid({key: minimum}, True)
            self.assertValid({key: minimum - 1}, False)
        self.assertValid({"width": 65536}, False)
        self.assertValid({"x": -1}, False)

    def test_shape_is_strict(self):
        for params in ({"pane": "7", "depth": 2}, {"title": 7},
                       {"focus": "yes"}, {"command": "htop"},
                       {"command": []}, {"command": [""]},
                       {"width": "wide"}, {"pane": True}):
            with self.subTest(params=params):
                self.assertValid(params, False)


if __name__ == "__main__":
    unittest.main()
