#!/usr/bin/env python3
import json
import sys
import os

def json_to_xml(json_template_path, output_path):
    with open(json_template_path, 'r') as f:
        template = json.load(f)
    
    scheme = template['scheme']
    
    xml_lines = []
    xml_lines.append('<?xml version="1.0" encoding="UTF-8"?>')
    xml_lines.append(f'<scheme name="$name" version="142" parent_scheme="${{theme == \'dark\' ? \'Darcula\' : \'Default\'}}">') 
    
    # MetaInfo
    xml_lines.append('  <metaInfo>')
    for prop in scheme['metaInfo']['property']:
        xml_lines.append(f'    <property name="{prop["+@name"]}">{prop["+content"]}</property>')
    xml_lines.append('  </metaInfo>')
    
    # Colors
    xml_lines.append('  <colors>')
    for name, value in scheme['colors'].items():
        xml_lines.append(f'    <option name="{name}" value="${{{value}}}" />')
    xml_lines.append('  </colors>')
    
    # Attributes
    xml_lines.append('  <attributes>')
    for attr in scheme['attributes']:
        attr_name = attr['+@name']
        xml_lines.append(f'    <option name="{attr_name}">')
        
        if 'attributes' in attr:
            xml_lines.append('      <value>')
            for prop_name, prop_value in attr['attributes'].items():
                xml_lines.append(f'        <option name="{prop_name}" value="${{{prop_value}}}" />')
            xml_lines.append('      </value>')
        else:
            xml_lines.append('      <value />')
            
        xml_lines.append('    </option>')
    xml_lines.append('  </attributes>')
    
    xml_lines.append('</scheme>')
    
    with open(output_path, 'w') as f:
        f.write('\n'.join(xml_lines))
    
    print(f"Created XML template at {output_path}")

if __name__ == "__main__":
    json_template_path = os.path.join(os.path.dirname(__file__), 'jetbrains.editor.template.json')
    output_path = os.path.join(os.path.dirname(__file__), 'jetbrains.editor.template.xml')
    
    json_to_xml(json_template_path, output_path)