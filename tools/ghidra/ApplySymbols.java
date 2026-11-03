// Applies re/symbols/count_exe.csv (functions) and re/symbols/count_exe_data.csv
// (DGROUP globals) to COUNT.EXE. Ambiguous matches get a "maybe_" prefix.
// Argument: repository root.
//@category OBCR
import java.io.BufferedReader;
import java.io.File;
import java.io.FileReader;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressIterator;
import ghidra.program.model.address.SegmentedAddress;
import ghidra.program.model.address.SegmentedAddressSpace;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.SourceType;

public class ApplySymbols extends GhidraScript {
	private static final int DGROUP_PARAGRAPH = 0x3218;

	@Override
	protected void run() throws Exception {
		File root = new File(getScriptArgs()[0]);
		AddressIterator it = currentProgram.getSymbolTable().getExternalEntryPointIterator();
		SegmentedAddress entry = (SegmentedAddress) it.next();
		SegmentedAddressSpace space = (SegmentedAddressSpace) entry.getAddressSpace();
		int dgroup = entry.getSegment() + DGROUP_PARAGRAPH;

		int funcs = 0;
		for (String[] row : rows(new File(root, "re/symbols/count_exe.csv"))) {
			String[] seg = row[1].split(":");
			Address a = space.getAddress(Integer.parseInt(seg[0], 16), Integer.parseInt(seg[1], 16));
			String name = clean(row[3]);
			if (row.length > 7 && !row[7].isEmpty()) {
				name = "maybe_" + name;
			}
			Function f = getFunctionAt(a);
			if (f != null) {
				f.setName(unique(name, a), SourceType.USER_DEFINED);
				funcs++;
			}
		}
		int data = 0;
		for (String[] row : rows(new File(root, "re/symbols/count_exe_data.csv"))) {
			int off = Integer.parseInt(row[0].substring(2), 16);
			Address a = space.getAddress(dgroup, off);
			createLabel(a, clean(row[1]).replace("+0x", "_plus_"), true, SourceType.USER_DEFINED);
			data++;
		}
		println(String.format("OBCR: named %d functions, %d globals", funcs, data));
	}

	private String unique(String name, Address a) {
		if (getSymbols(name, null).isEmpty()) {
			return name;
		}
		return name + "_" + a.toString().replace(":", "_");
	}

	private static String clean(String name) {
		if (name.startsWith("_") && !name.startsWith("__")) {
			return name.substring(1);
		}
		return name;
	}

	private static java.util.List<String[]> rows(File csv) throws Exception {
		java.util.List<String[]> out = new java.util.ArrayList<>();
		try (BufferedReader r = new BufferedReader(new FileReader(csv))) {
			String line = r.readLine();
			while ((line = r.readLine()) != null) {
				if (!line.isBlank()) {
					out.add(line.split(",", -1));
				}
			}
		}
		return out;
	}
}
