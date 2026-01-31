#!/usr/bin/env python3
"""
Design Document Converter for Plan Cascade

Converts external design documents (Markdown, JSON, HTML) into the
unified design_doc.json format for use with Plan Cascade execution.
"""

import json
import re
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


class DesignDocConverter:
    """Converts external design documents to design_doc.json format."""

    def __init__(self, project_root: Path):
        """
        Initialize the design document converter.

        Args:
            project_root: Root directory of the project
        """
        self.project_root = Path(project_root)
        self.design_doc_path = self.project_root / "design_doc.json"

    def convert(self, input_path: Path | str, format_type: str | None = None) -> dict:
        """
        Convert an external document to design_doc.json format.

        Args:
            input_path: Path to the input document
            format_type: Format type (markdown, json, html) - auto-detected if not provided

        Returns:
            Design document dictionary
        """
        input_path = Path(input_path)

        if not input_path.exists():
            raise FileNotFoundError(f"Input file not found: {input_path}")

        # Auto-detect format
        if format_type is None:
            format_type = self._detect_format(input_path)

        # Read file content
        with open(input_path, encoding="utf-8") as f:
            content = f.read()

        # Convert based on format
        if format_type == "markdown":
            return self._convert_markdown(content, input_path)
        elif format_type == "json":
            return self._convert_json(content, input_path)
        elif format_type == "html":
            return self._convert_html(content, input_path)
        else:
            raise ValueError(f"Unsupported format: {format_type}")

    def _detect_format(self, path: Path) -> str:
        """Detect document format from file extension."""
        suffix = path.suffix.lower()
        format_map = {
            ".md": "markdown",
            ".markdown": "markdown",
            ".json": "json",
            ".html": "html",
            ".htm": "html"
        }
        return format_map.get(suffix, "markdown")

    def _convert_markdown(self, content: str, source_path: Path) -> dict:
        """
        Convert Markdown document to design_doc.json format.

        Parses headings to extract structure:
        - # Title -> overview.title
        - ## Overview/Summary -> overview.summary
        - ## Architecture -> architecture section
        - ## Decisions/ADR -> decisions section
        - etc.

        Args:
            content: Markdown content
            source_path: Source file path

        Returns:
            Design document dictionary
        """
        design_doc = self._create_base_doc(str(source_path))

        # Parse sections by headings
        sections = self._parse_markdown_sections(content)

        # Extract title from first H1
        if sections.get("h1"):
            design_doc["overview"]["title"] = sections["h1"][0]["title"]
            if sections["h1"][0]["content"]:
                design_doc["overview"]["summary"] = sections["h1"][0]["content"].strip()

        # Process H2 sections
        for section in sections.get("h2", []):
            title_lower = section["title"].lower()

            if any(k in title_lower for k in ["overview", "summary", "introduction"]):
                self._parse_overview_section(design_doc, section["content"])

            elif any(k in title_lower for k in ["architecture", "design", "structure"]):
                self._parse_architecture_section(design_doc, section["content"])

            elif any(k in title_lower for k in ["decision", "adr", "rationale"]):
                self._parse_decisions_section(design_doc, section["content"])

            elif any(k in title_lower for k in ["api", "endpoint", "interface"]):
                self._parse_api_section(design_doc, section["content"])

            elif any(k in title_lower for k in ["goal", "objective"]):
                goals = self._extract_list_items(section["content"])
                design_doc["overview"]["goals"] = goals

            elif any(k in title_lower for k in ["non-goal", "out of scope", "limitations"]):
                non_goals = self._extract_list_items(section["content"])
                design_doc["overview"]["non_goals"] = non_goals

            elif any(k in title_lower for k in ["component", "module"]):
                self._parse_components_section(design_doc, section["content"])

            elif any(k in title_lower for k in ["pattern", "approach"]):
                self._parse_patterns_section(design_doc, section["content"])

            elif any(k in title_lower for k in ["data model", "schema", "entity"]):
                self._parse_data_models_section(design_doc, section["content"])

        return design_doc

    def _parse_markdown_sections(self, content: str) -> dict[str, list[dict]]:
        """Parse markdown into sections by heading level."""
        sections: dict[str, list[dict]] = {"h1": [], "h2": [], "h3": []}

        # Split by headings
        lines = content.split("\n")
        current_level = None
        current_title = ""
        current_content: list[str] = []

        for line in lines:
            h1_match = re.match(r"^#\s+(.+)$", line)
            h2_match = re.match(r"^##\s+(.+)$", line)
            h3_match = re.match(r"^###\s+(.+)$", line)

            if h1_match or h2_match or h3_match:
                # Save previous section
                if current_level:
                    sections[current_level].append({
                        "title": current_title,
                        "content": "\n".join(current_content).strip()
                    })

                # Start new section
                if h1_match:
                    current_level = "h1"
                    current_title = h1_match.group(1).strip()
                elif h2_match:
                    current_level = "h2"
                    current_title = h2_match.group(1).strip()
                else:
                    current_level = "h3"
                    current_title = h3_match.group(1).strip()

                current_content = []
            else:
                current_content.append(line)

        # Save last section
        if current_level:
            sections[current_level].append({
                "title": current_title,
                "content": "\n".join(current_content).strip()
            })

        return sections

    def _parse_overview_section(self, design_doc: dict, content: str) -> None:
        """Parse overview section content."""
        if not design_doc["overview"]["summary"]:
            # Take first paragraph as summary
            paragraphs = content.split("\n\n")
            if paragraphs:
                design_doc["overview"]["summary"] = paragraphs[0].strip()

    def _parse_architecture_section(self, design_doc: dict, content: str) -> None:
        """Parse architecture section content."""
        # Look for data flow description
        if "data flow" in content.lower() or "flow" in content.lower():
            # Extract paragraph about flow
            paragraphs = content.split("\n\n")
            for para in paragraphs:
                if "flow" in para.lower():
                    design_doc["architecture"]["data_flow"] = para.strip()
                    break

    def _parse_decisions_section(self, design_doc: dict, content: str) -> None:
        """Parse decisions/ADR section content."""
        # Look for numbered decisions or ADR patterns
        adr_pattern = r"(?:ADR[-\s]?\d+|Decision\s*\d+)[:\s]+(.+?)(?=(?:ADR[-\s]?\d+|Decision\s*\d+)|$)"
        matches = re.findall(adr_pattern, content, re.IGNORECASE | re.DOTALL)

        for i, match in enumerate(matches):
            adr_id = f"ADR-{i + 1:03d}"
            # Extract title from first line
            lines = match.strip().split("\n")
            title = lines[0].strip() if lines else f"Decision {i + 1}"

            design_doc["decisions"].append({
                "id": adr_id,
                "title": title,
                "context": "",
                "decision": match.strip(),
                "rationale": "",
                "alternatives_considered": [],
                "status": "accepted"
            })

    def _parse_api_section(self, design_doc: dict, content: str) -> None:
        """Parse API section content."""
        # Look for HTTP method + path patterns
        api_pattern = r"(GET|POST|PUT|DELETE|PATCH)\s+([/\w\-{}:]+)"
        matches = re.findall(api_pattern, content, re.IGNORECASE)

        for i, (method, path) in enumerate(matches):
            api_id = f"API-{i + 1:03d}"
            design_doc["interfaces"]["apis"].append({
                "id": api_id,
                "method": method.upper(),
                "path": path,
                "description": "",
                "request_body": {},
                "response": {}
            })

    def _parse_components_section(self, design_doc: dict, content: str) -> None:
        """Parse components section content."""
        # Look for component names (capitalized words or items in lists)
        list_items = self._extract_list_items(content)
        for item in list_items:
            # Extract component name (first word or phrase before description)
            parts = re.split(r"[-:,]", item, maxsplit=1)
            name = parts[0].strip()
            description = parts[1].strip() if len(parts) > 1 else ""

            if name:
                design_doc["architecture"]["components"].append({
                    "name": name,
                    "description": description,
                    "responsibilities": [],
                    "dependencies": [],
                    "files": []
                })

    def _parse_patterns_section(self, design_doc: dict, content: str) -> None:
        """Parse patterns section content."""
        list_items = self._extract_list_items(content)
        for item in list_items:
            parts = re.split(r"[-:,]", item, maxsplit=1)
            name = parts[0].strip()
            description = parts[1].strip() if len(parts) > 1 else ""

            if name:
                design_doc["architecture"]["patterns"].append({
                    "name": name,
                    "description": description,
                    "rationale": ""
                })

    def _parse_data_models_section(self, design_doc: dict, content: str) -> None:
        """Parse data models section content."""
        list_items = self._extract_list_items(content)
        for item in list_items:
            parts = re.split(r"[-:,]", item, maxsplit=1)
            name = parts[0].strip()
            description = parts[1].strip() if len(parts) > 1 else ""

            if name:
                design_doc["interfaces"]["data_models"].append({
                    "name": name,
                    "description": description,
                    "fields": {}
                })

    def _extract_list_items(self, content: str) -> list[str]:
        """Extract list items from content."""
        items = []
        for line in content.split("\n"):
            # Match bullet points
            bullet_match = re.match(r"^\s*[-*+]\s+(.+)$", line)
            if bullet_match:
                items.append(bullet_match.group(1).strip())
            # Match numbered items
            number_match = re.match(r"^\s*\d+[.)]\s+(.+)$", line)
            if number_match:
                items.append(number_match.group(1).strip())
        return items

    def _convert_json(self, content: str, source_path: Path) -> dict:
        """
        Convert JSON document to design_doc.json format.

        Handles both:
        - Direct design_doc format (validates and returns)
        - Custom JSON formats (maps fields)

        Args:
            content: JSON content
            source_path: Source file path

        Returns:
            Design document dictionary
        """
        try:
            data = json.loads(content)
        except json.JSONDecodeError as e:
            raise ValueError(f"Invalid JSON: {e}") from e

        # Check if it's already in design_doc format
        if self._is_design_doc_format(data):
            data["metadata"]["source"] = "converted"
            return data

        # Otherwise, try to map fields
        design_doc = self._create_base_doc(str(source_path))

        # Map common field names
        field_mappings = {
            "title": ("overview", "title"),
            "name": ("overview", "title"),
            "summary": ("overview", "summary"),
            "description": ("overview", "summary"),
            "goals": ("overview", "goals"),
            "objectives": ("overview", "goals"),
            "non_goals": ("overview", "non_goals"),
            "out_of_scope": ("overview", "non_goals"),
            "components": ("architecture", "components"),
            "patterns": ("architecture", "patterns"),
            "data_flow": ("architecture", "data_flow"),
            "apis": ("interfaces", "apis"),
            "endpoints": ("interfaces", "apis"),
            "data_models": ("interfaces", "data_models"),
            "models": ("interfaces", "data_models"),
            "decisions": ("decisions", None),
            "adrs": ("decisions", None),
        }

        for json_key, (section, field) in field_mappings.items():
            if json_key in data:
                if field is None:
                    design_doc[section] = data[json_key]
                else:
                    design_doc[section][field] = data[json_key]

        return design_doc

    def _is_design_doc_format(self, data: dict) -> bool:
        """Check if data is already in design_doc format."""
        required_sections = ["metadata", "overview", "architecture", "decisions"]
        return all(section in data for section in required_sections)

    def _convert_html(self, content: str, source_path: Path) -> dict:
        """
        Convert HTML document to design_doc.json format.

        Handles Confluence/Notion exports by parsing HTML structure.

        Args:
            content: HTML content
            source_path: Source file path

        Returns:
            Design document dictionary
        """
        design_doc = self._create_base_doc(str(source_path))

        # Simple HTML parsing without external dependencies
        # Extract title from <title> or <h1>
        title_match = re.search(r"<title>(.+?)</title>", content, re.IGNORECASE)
        if title_match:
            design_doc["overview"]["title"] = self._strip_html_tags(title_match.group(1))

        h1_match = re.search(r"<h1[^>]*>(.+?)</h1>", content, re.IGNORECASE | re.DOTALL)
        if h1_match:
            design_doc["overview"]["title"] = self._strip_html_tags(h1_match.group(1))

        # Extract sections from h2 tags
        h2_pattern = r"<h2[^>]*>(.+?)</h2>(.*?)(?=<h2|<h1|$)"
        for match in re.finditer(h2_pattern, content, re.IGNORECASE | re.DOTALL):
            section_title = self._strip_html_tags(match.group(1)).lower()
            section_content = self._strip_html_tags(match.group(2))

            if any(k in section_title for k in ["overview", "summary", "introduction"]):
                paragraphs = section_content.split("\n\n")
                if paragraphs:
                    design_doc["overview"]["summary"] = paragraphs[0].strip()

            elif any(k in section_title for k in ["goal", "objective"]):
                items = self._extract_list_from_text(section_content)
                design_doc["overview"]["goals"] = items

        # Extract list items from <ul>/<ol>
        list_pattern = r"<li[^>]*>(.+?)</li>"
        for match in re.finditer(list_pattern, content, re.IGNORECASE | re.DOTALL):
            item = self._strip_html_tags(match.group(1)).strip()
            # Add to goals if not already present
            if item and item not in design_doc["overview"]["goals"]:
                design_doc["overview"]["goals"].append(item)

        return design_doc

    def _strip_html_tags(self, text: str) -> str:
        """Remove HTML tags from text."""
        clean = re.sub(r"<[^>]+>", "", text)
        # Decode common HTML entities
        clean = clean.replace("&amp;", "&")
        clean = clean.replace("&lt;", "<")
        clean = clean.replace("&gt;", ">")
        clean = clean.replace("&quot;", '"')
        clean = clean.replace("&#39;", "'")
        clean = clean.replace("&nbsp;", " ")
        return clean.strip()

    def _extract_list_from_text(self, text: str) -> list[str]:
        """Extract list items from plain text."""
        items = []
        for line in text.split("\n"):
            line = line.strip()
            # Match bullet/number patterns
            match = re.match(r"^[-*+•]?\s*\d*[.)]*\s*(.+)$", line)
            if match and match.group(1).strip():
                items.append(match.group(1).strip())
        return items

    def _create_base_doc(self, source: str) -> dict:
        """Create base design document structure."""
        return {
            "metadata": {
                "created_at": datetime.now(timezone.utc).isoformat(),
                "version": "1.0.0",
                "source": "converted",
                "prd_reference": None,
                "original_file": source
            },
            "overview": {
                "title": "",
                "summary": "",
                "goals": [],
                "non_goals": []
            },
            "architecture": {
                "components": [],
                "data_flow": "",
                "patterns": []
            },
            "interfaces": {
                "apis": [],
                "data_models": []
            },
            "decisions": [],
            "story_mappings": {}
        }

    def save_design_doc(self, design_doc: dict) -> bool:
        """
        Save design document to file.

        Args:
            design_doc: Design document dictionary

        Returns:
            True if saved successfully
        """
        try:
            with open(self.design_doc_path, "w", encoding="utf-8") as f:
                json.dump(design_doc, f, indent=2)
            return True
        except OSError as e:
            print(f"Error saving design document: {e}")
            return False


def main():
    """CLI interface for testing design document converter."""
    if len(sys.argv) < 2:
        print("Usage: design_doc_converter.py <command> [args]")
        print("Commands:")
        print("  convert <input_file>  - Convert file to design_doc.json")
        print("  detect <input_file>   - Detect file format")
        sys.exit(1)

    command = sys.argv[1]
    project_root = Path.cwd()

    converter = DesignDocConverter(project_root)

    if command == "convert" and len(sys.argv) >= 3:
        input_path = Path(sys.argv[2])
        try:
            design_doc = converter.convert(input_path)
            if converter.save_design_doc(design_doc):
                print(f"Converted {input_path} to design_doc.json")
                print(json.dumps(design_doc, indent=2))
            else:
                print("Failed to save design document")
                sys.exit(1)
        except (FileNotFoundError, ValueError) as e:
            print(f"Error: {e}")
            sys.exit(1)

    elif command == "detect" and len(sys.argv) >= 3:
        input_path = Path(sys.argv[2])
        format_type = converter._detect_format(input_path)
        print(f"Detected format: {format_type}")

    else:
        print(f"Unknown command: {command}")
        sys.exit(1)


if __name__ == "__main__":
    main()
