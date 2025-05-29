import xml.etree.ElementTree as ET
import re

def parse_keys_file(keys_file):
    """Parse the keys.txt file and return a dictionary of key-value pairs."""
    keys_dict = {}
    with open(keys_file, 'r') as f:
        for line in f:
            line = line.strip()
            if line and ',' in line:
                key, value = line.split(',', 1)
                keys_dict[key.strip()] = value.strip()
    return keys_dict

def update_xml_colors(xml_file, keys_dict, output_file):
    """Update the XML file with new color values from keys_dict and remove non-existent keys."""

    # Parse the XML file
    tree = ET.parse(xml_file)
    root = tree.getroot()

    # Color attributes that we want to update
    color_attributes = ['FOREGROUND', 'BACKGROUND', 'EFFECT_COLOR']

    # Find the attributes element (parent of all option elements)
    attributes_elem = root.find('.//attributes')
    if attributes_elem is None:
        print("Warning: No 'attributes' element found in XML")
        return

    # Collect all option elements to process
    options_to_remove = []
    options_updated = 0

    # Find all option elements within attributes
    for option in attributes_elem.findall('option[@name]'):
        option_name = option.get('name')

        # Check if this option name exists in our keys dictionary
        if option_name in keys_dict:
            new_color_value = keys_dict[option_name]

            # Find the value element within this option
            value_elem = option.find('value')
            if value_elem is not None:
                # Find all nested option elements within the value
                color_updated = False
                for nested_option in value_elem.findall('option'):
                    nested_name = nested_option.get('name')

                    # If this is a color attribute, update its value
                    if nested_name in color_attributes:
                        nested_option.set('value', new_color_value)
                        color_updated = True

                if color_updated:
                    options_updated += 1
                    print(f"Updated: {option_name}")
        else:
            # This key doesn't exist in keys.txt, mark for removal
            options_to_remove.append(option)

    # Remove options that don't exist in keys.txt
    for option in options_to_remove:
        option_name = option.get('name')
        attributes_elem.remove(option)
        print(f"Removed: {option_name}")

    print(f"\nSummary:")
    print(f"- Updated {options_updated} existing keys")
    print(f"- Removed {len(options_to_remove)} keys not in keys.txt")

    # Write the modified XML to output file
    tree.write(output_file, encoding='utf-8', xml_declaration=True)
    print(f"Updated XML saved to {output_file}")

def main():
    # File paths
    keys_file = 'keys.txt'
    xml_file = 'cattest.xml'  # Input XML file
    output_file = 'cattest_updated.xml'  # Output XML file

    try:
        # Parse the keys file
        print("Parsing keys file...")
        keys_dict = parse_keys_file(keys_file)
        print(f"Found {len(keys_dict)} keys in keys.txt")

        # Update the XML file
        print("\nProcessing XML file...")
        update_xml_colors(xml_file, keys_dict, output_file)

        print("\nDone!")

    except FileNotFoundError as e:
        print(f"Error: File not found - {e}")
    except Exception as e:
        print(f"Error: {e}")

if __name__ == "__main__":
    main()
