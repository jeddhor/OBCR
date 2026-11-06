// Exports functions, decompilations, disassembly and strings of COUNT.EXE as text
// so agents can grep them without the GUI. Output goes to .local/ghidra/export
// (never commit it: it is derived from the original program).
// Argument: output directory.
//@category OBCR
import java.io.File;
import java.io.FileWriter;
import java.io.PrintWriter;
import java.util.Set;
import java.util.TreeSet;

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileOptions;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressIterator;
import ghidra.program.model.address.SegmentedAddress;
import ghidra.program.model.listing.Data;
import ghidra.program.model.listing.DataIterator;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;
import ghidra.program.model.listing.Listing;

public class ExportAll extends GhidraScript {
	private static final int DGROUP_PARAGRAPH = 0x3218;
	private static final long HEADER_BYTES = 0x3400;

	@Override
	protected void run() throws Exception {
		String[] args = getScriptArgs();
		File out = new File(args.length > 0 ? args[0] : "export");
		File decompDir = new File(out, "decomp");
		File asmDir = new File(out, "asm");
		decompDir.mkdirs();
		asmDir.mkdirs();
		int base = baseSegment();

		DecompInterface ifc = new DecompInterface();
		ifc.setOptions(new DecompileOptions());
		ifc.openProgram(currentProgram);
		Listing listing = currentProgram.getListing();

		try (PrintWriter tsv = new PrintWriter(new FileWriter(new File(out, "functions.tsv")))) {
			tsv.println("ghidra_addr\timg_linear\tfile_off\tname\tsize\tcallers\tcallees");
			for (Function f : currentProgram.getFunctionManager().getFunctions(true)) {
				if (monitor.isCancelled()) {
					break;
				}
				Address entry = f.getEntryPoint();
				long lin = imageLinear(entry, base);
				String stem = String.format("%05X_%s", lin, sanitize(f.getName()));
				String header = String.format("// %s  %s  img+0x%05X  file 0x%05X%n", f.getName(),
					entry, lin, lin + HEADER_BYTES);

				DecompileResults r = ifc.decompileFunction(f, 120, monitor);
				String c = r.decompileCompleted() ? r.getDecompiledFunction().getC()
						: "/* decompile failed: " + r.getErrorMessage() + " */\n";
				try (PrintWriter w = new PrintWriter(new FileWriter(new File(decompDir, stem + ".c")))) {
					w.print(header);
					w.print(c);
				}
				try (PrintWriter w = new PrintWriter(new FileWriter(new File(asmDir, stem + ".asm")))) {
					w.print(header);
					InstructionIterator ii = listing.getInstructions(f.getBody(), true);
					while (ii.hasNext()) {
						Instruction ins = ii.next();
						w.printf("%s  %05X  %s%n", ins.getAddress(), imageLinear(ins.getAddress(), base),
							ins);
					}
				}
				tsv.printf("%s\t0x%05X\t0x%05X\t%s\t%d\t%s\t%s%n", entry, lin, lin + HEADER_BYTES,
					f.getName(), f.getBody().getNumAddresses(), names(f.getCallingFunctions(monitor)),
					names(f.getCalledFunctions(monitor)));
			}
		}

		try (PrintWriter w = new PrintWriter(new FileWriter(new File(out, "strings.tsv")))) {
			w.println("ghidra_addr\timg_linear\tfile_off\tds_off\ttext");
			DataIterator di = listing.getDefinedData(true);
			while (di.hasNext()) {
				Data d = di.next();
				if (!d.hasStringValue()) {
					continue;
				}
				long lin = imageLinear(d.getAddress(), base);
				long dsOff = lin - ((long) DGROUP_PARAGRAPH << 4);
				w.printf("%s\t0x%05X\t0x%05X\t%s\t%s%n", d.getAddress(), lin, lin + HEADER_BYTES,
					dsOff >= 0 ? String.format("0x%04X", dsOff) : "", String.valueOf(d.getValue())
							.replace("\t", "\\t").replace("\n", "\\n").replace("\r", "\\r"));
			}
		}
		ifc.dispose();
		println("OBCR: export written to " + out.getAbsolutePath());
	}

	private int baseSegment() {
		AddressIterator it = currentProgram.getSymbolTable().getExternalEntryPointIterator();
		return ((SegmentedAddress) it.next()).getSegment();
	}

	private static long imageLinear(Address a, int base) {
		SegmentedAddress s = (SegmentedAddress) a;
		return ((long) (s.getSegment() - base) << 4) + s.getSegmentOffset();
	}

	private static String sanitize(String name) {
		return name.replaceAll("[^A-Za-z0-9_@$]", "_");
	}

	private static String names(Set<Function> fs) {
		Set<String> sorted = new TreeSet<>();
		for (Function f : fs) {
			sorted.add(f.getName());
		}
		return String.join(",", sorted);
	}
}
