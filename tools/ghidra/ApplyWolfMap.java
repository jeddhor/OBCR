// Names WOLF3D.EXE from its Borland linker map (WOLFSRC/WOLF3D.MAP) and sets
// DS/SS to its DGROUP. Run as a -preScript when importing WOLF3D.EXE.
// Arguments: path to WOLF3D.MAP, DGROUP paragraph in hex (3A39).
//@category OBCR
import java.io.BufferedReader;
import java.io.FileReader;
import java.math.BigInteger;
import java.util.ArrayList;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import ghidra.app.cmd.disassemble.DisassembleCommand;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressIterator;
import ghidra.program.model.address.SegmentedAddress;
import ghidra.program.model.address.SegmentedAddressSpace;
import ghidra.program.model.lang.Register;
import ghidra.program.model.listing.ProgramContext;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.symbol.SourceType;

public class ApplyWolfMap extends GhidraScript {
	private static final Pattern SEGMENT =
		Pattern.compile("^\\s*([0-9A-F]{5})H\\s+([0-9A-F]{5})H\\s+[0-9A-F]{5}H\\s+(\\S+)\\s+(\\S+)");
	private static final Pattern PUBLIC =
		Pattern.compile("^\\s*([0-9A-F]{4}):([0-9A-F]{4})\\s+(?:idle|Abs)?\\s*(\\S+)\\s*$");

	@Override
	protected void run() throws Exception {
		String[] args = getScriptArgs();
		String mapPath = args[0];
		int dgroup = Integer.parseInt(args[1], 16);
		AddressIterator it = currentProgram.getSymbolTable().getExternalEntryPointIterator();
		SegmentedAddress entry = (SegmentedAddress) it.next();
		int base = entry.getSegment();
		SegmentedAddressSpace space = (SegmentedAddressSpace) entry.getAddressSpace();

		ProgramContext ctx = currentProgram.getProgramContext();
		Register ds = ctx.getRegister("DS");
		Register ss = ctx.getRegister("SS");
		for (MemoryBlock b : currentProgram.getMemory().getBlocks()) {
			ctx.setValue(ds, b.getStart(), b.getEnd(), BigInteger.valueOf(base + dgroup));
			ctx.setValue(ss, b.getStart(), b.getEnd(), BigInteger.valueOf(base + dgroup));
		}

		List<long[]> code = new ArrayList<>();
		boolean publics = false;
		int named = 0;
		int functions = 0;
		try (BufferedReader r = new BufferedReader(new FileReader(mapPath))) {
			String line;
			while ((line = r.readLine()) != null) {
				Matcher seg = SEGMENT.matcher(line);
				if (!publics && seg.find() && seg.group(4).equals("CODE")) {
					code.add(new long[] { Long.parseLong(seg.group(1), 16), Long.parseLong(seg.group(2), 16) });
				}
				if (line.contains("Publics by Value")) {
					publics = true;
					continue;
				}
				if (!publics) {
					continue;
				}
				Matcher m = PUBLIC.matcher(line);
				if (!m.find() || line.contains("Abs")) {
					continue;
				}
				int s = Integer.parseInt(m.group(1), 16);
				int o = Integer.parseInt(m.group(2), 16);
				String name = m.group(3);
				long linear = ((long) s << 4) + o;
				Address a = space.getAddress(base + s, o);
				if (currentProgram.getMemory().getBlock(a) == null) {
					continue;
				}
				createLabel(a, name, true, SourceType.IMPORTED);
				named++;
				boolean isCode = false;
				for (long[] c : code) {
					isCode |= linear >= c[0] && linear <= c[1];
				}
				if (isCode && getFunctionAt(a) == null) {
					new DisassembleCommand(a, null, true).applyTo(currentProgram, monitor);
					if (createFunction(a, name) != null) {
						functions++;
					}
				}
			}
		}
		println(String.format("OBCR: base %04X, DS=%04X, %d names, %d new functions, %d code segments",
			base, base + dgroup, named, functions, code.size()));
	}
}
