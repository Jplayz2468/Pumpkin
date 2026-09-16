// Invokes vanilla's real StepBasedCollector. The probe only records primary effects.
import java.util.*;
import java.lang.reflect.Field;
import net.minecraft.world.entity.*;
import net.minecraft.network.syncher.SynchedEntityData;
import net.minecraft.world.level.storage.ValueInput;
import net.minecraft.world.level.storage.ValueOutput;
import net.minecraft.world.damagesource.DamageSource;
import net.minecraft.server.level.ServerLevel;
import sun.misc.Unsafe;

public class InsideEffectsOracle {
    static final List<String> trace = new ArrayList<>();
    static class Probe extends Entity {
        Probe() { super(null, null); }
        public boolean isAlive() { return true; }
        public void setIsInPowderSnow(boolean value) { trace.add("e0"); }
        public boolean canFreeze() { return false; }
        public void clearFreeze() { trace.add("e1"); }
        public boolean fireImmune() { trace.add("e2"); return true; }
        public void lavaIgnite() { trace.add("e3"); }
        public void clearFire() { trace.add("e4"); }
        public boolean hurtServer(ServerLevel level, DamageSource source, float damage) { return false; }
        protected void defineSynchedData(SynchedEntityData.Builder builder) {}
        protected void readAdditionalSaveData(ValueInput input) {}
        protected void addAdditionalSaveData(ValueOutput output) {}
    }
    public static void main(String[] args) throws Exception {
        java.io.PrintStream output = System.out;
        net.minecraft.SharedConstants.tryDetectVersion();
        net.minecraft.server.Bootstrap.bootStrap();
        Field field = Unsafe.class.getDeclaredField("theUnsafe"); field.setAccessible(true);
        Probe probe = (Probe)((Unsafe)field.get(null)).allocateInstance(Probe.class);
        Random random = new Random(262);
        output.println("[");
        for (int c = 0; c < 100; c++) {
            trace.clear();
            InsideBlockEffectApplier.StepBasedCollector collector = new InsideBlockEffectApplier.StepBasedCollector();
            StringJoiner operations = new StringJoiner(",");
            for (int i = 0; i < 80; i++) {
                int kind = random.nextInt(5), step = random.nextInt(5), type = random.nextInt(5);
                String token = "c" + i;
                operations.add("[" + kind + "," + step + "," + type + "," + i + "]");
                switch (kind) {
                    case 0 -> collector.advanceStep(step);
                    case 1 -> collector.apply(InsideBlockEffectType.values()[type]);
                    case 2 -> collector.runBefore(InsideBlockEffectType.values()[type], e -> trace.add(token));
                    case 3 -> collector.runAfter(InsideBlockEffectType.values()[type], e -> trace.add(token));
                    case 4 -> collector.applyAndClear(probe);
                }
            }
            collector.applyAndClear(probe);
            StringJoiner result = new StringJoiner(",");
            for (String entry : trace) result.add("\"" + entry + "\"");
            output.println((c == 0 ? "" : ",") + "{\"operations\":[" + operations + "],\"trace\":[" + result + "]}");
        }
        output.println("]");
    }
}
