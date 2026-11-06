// Sets DS and SS to DGROUP over all of COUNT.EXE so data references resolve.
// Run as a -preScript, before auto-analysis.
//@category OBCR
import java.math.BigInteger;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressIterator;
import ghidra.program.model.address.SegmentedAddress;
import ghidra.program.model.lang.Register;
import ghidra.program.model.listing.ProgramContext;
import ghidra.program.model.mem.MemoryBlock;

public class SetDgroup extends GhidraScript {
	/** DGROUP image paragraph for release OBC-A (docs/re/SYMBOLS.md). */
	private static final int DGROUP_PARAGRAPH = 0x3218;

	@Override
	protected void run() throws Exception {
		int base = ObcAddr.baseSegment(currentProgram);
		int dgroup = base + DGROUP_PARAGRAPH;
		ProgramContext ctx = currentProgram.getProgramContext();
		Register ds = ctx.getRegister("DS");
		Register ss = ctx.getRegister("SS");
		for (MemoryBlock block : currentProgram.getMemory().getBlocks()) {
			ctx.setValue(ds, block.getStart(), block.getEnd(), BigInteger.valueOf(dgroup));
			ctx.setValue(ss, block.getStart(), block.getEnd(), BigInteger.valueOf(dgroup));
		}
		println(String.format("OBCR: base segment %04X, DS=SS=%04X", base, dgroup));
	}
}

/** Address helpers shared by the OBCR scripts. */
class ObcAddr {
	/** Ghidra's load segment: the entry point is image 0000:0000 (header CS:IP). */
	static int baseSegment(ghidra.program.model.listing.Program program) {
		AddressIterator it = program.getSymbolTable().getExternalEntryPointIterator();
		if (!it.hasNext()) {
			throw new IllegalStateException("no entry point");
		}
		Address entry = it.next();
		return ((SegmentedAddress) entry).getSegment();
	}

	static long imageLinear(Address a, int base) {
		SegmentedAddress s = (SegmentedAddress) a;
		return ((long) (s.getSegment() - base) << 4) + s.getSegmentOffset();
	}
}
