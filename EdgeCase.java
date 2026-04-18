public class EdgeCase {
    public static void main(String[] args) {
        try {
            execute();
        } catch (Exception e) {
            e.printStackTrace();
        }
    }

    public static void execute() {
        inner(); // Line 10
    }

    public static void inner() {
        String data = null;
        System.out.println(data.length()); // Line 15 - NullPointerException
    }
}
