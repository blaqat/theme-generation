"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { AlertCircle } from "lucide-react";

type DiffResult = {
	key: string;
	type: "added" | "changed";
	value1?: any;
	value2?: any;
};

export default function JSONDiffViewer() {
	const [json1, setJson1] = useState("");
	const [json2, setJson2] = useState("");
	const [diff, setDiff] = useState<DiffResult[]>([]);
	const [error, setError] = useState<string | null>(null);

	const normalizeHexColor = (hex: string): string => {
		// Remove the hash if it exists
		hex = hex.replace(/^#/, "").toLowerCase();
		window;

		// Expand short hex to full hex
		if (hex.length === 3) {
			hex = hex
				.split("")
				.map((char) => char + char)
				.join("");
		}

		// Normalize 8-digit hex (convert alpha to lowercase)
		if (hex.length === 8) {
			return hex.slice(0, 6) + hex.slice(6).toLowerCase();
		}

		return hex;
	};

	const normalizeValue = (value: any): any => {
		if (typeof value === "string") {
			if (value.startsWith("#")) {
				return normalizeHexColor(value);
			}
			return value.toLowerCase();
		}
		return value;
	};

	const compareJSON = (
		obj1: any,
		obj2: any,
		path: string = "",
	): DiffResult[] => {
		const result: DiffResult[] = [];

		for (const key in obj1) {
			const fullPath = path ? `${path}.${key}` : key;

			if (!(key in obj2)) {
				result.push({
					key: fullPath,
					type: "changed",
					value1: obj1[key],
					value2: undefined,
				});
			} else if (
				typeof obj1[key] === "object" &&
				obj1[key] !== null &&
				typeof obj2[key] === "object" &&
				obj2[key] !== null
			) {
				result.push(...compareJSON(obj1[key], obj2[key], fullPath));
			} else if (normalizeValue(obj1[key]) !== normalizeValue(obj2[key])) {
				result.push({
					key: fullPath,
					type: "changed",
					value1: obj1[key],
					value2: obj2[key],
				});
			}
		}

		for (const key in obj2) {
			const fullPath = path ? `${path}.${key}` : key;
			if (!(key in obj1)) {
				result.push({
					key: fullPath,
					type: "added",
					value1: undefined,
					value2: obj2[key],
				});
			}
		}

		return result;
	};

	const handleCompare = () => {
		try {
			const parsedJson1 = JSON.parse(json1);
			const parsedJson2 = JSON.parse(json2);
			const diffResult = compareJSON(parsedJson1, parsedJson2);
			setDiff(diffResult);
			setError(null);
		} catch (err) {
			setError("Invalid JSON input. Please check your JSON and try again.");
			setDiff([]);
		}
	};

	return (
		<div className="container mx-auto p-4 max-w-4xl">
			<h1 className="text-2xl font-bold mb-4">JSON Diff Viewer</h1>
			<div className="grid grid-cols-1 md:grid-cols-2 gap-4 mb-4">
				<div>
					<label
						htmlFor="json1"
						className="block text-sm font-medium text-gray-700 mb-2"
					>
						JSON 1
					</label>
					<Textarea
						id="json1"
						value={json1}
						onChange={(e) => setJson1(e.target.value)}
						placeholder="Paste your first JSON here"
						className="h-64"
					/>
				</div>
				<div>
					<label
						htmlFor="json2"
						className="block text-sm font-medium text-gray-700 mb-2"
					>
						JSON 2
					</label>
					<Textarea
						id="json2"
						value={json2}
						onChange={(e) => setJson2(e.target.value)}
						placeholder="Paste your second JSON here"
						className="h-64"
					/>
				</div>
			</div>
			<Button onClick={handleCompare} className="mb-4">
				Compare JSON
			</Button>
			{error && (
				<Alert variant="destructive" className="mb-4">
					<AlertCircle className="h-4 w-4" />
					<AlertTitle>Error</AlertTitle>
					<AlertDescription>{error}</AlertDescription>
				</Alert>
			)}
			{diff.length > 0 && (
				<div className="border rounded-lg p-4 bg-white">
					<h2 className="text-xl font-semibold mb-2">Diff Result</h2>
					<pre className="whitespace-pre-wrap">
						{diff.map((item, index) => (
							<div
								key={index}
								className={`${
									item.type === "added" ? "bg-green-100" : "bg-yellow-100"
								} p-1 mb-1 rounded`}
							>
								<span className="font-semibold">{item.key}: </span>
								{item.type === "added" ? (
									<span className="text-green-600">
										Added: {JSON.stringify(item.value2)}
									</span>
								) : (
									<>
										<span className="text-red-600">
											Old: {JSON.stringify(item.value1)}
										</span>
										<br />
										<span className="text-green-600 ml-4">
											New: {JSON.stringify(item.value2)}
										</span>
									</>
								)}
							</div>
						))}
					</pre>
				</div>
			)}
		</div>
	);
}
