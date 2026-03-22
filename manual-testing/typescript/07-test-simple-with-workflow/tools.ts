// In-memory database simulation for testing
import type { User, Product, Order, MainTools } from "./generated/main.agent.types";

// ═══════════════════════════════════════════════════════════════════════════
// DATABASE STORAGE
// ═══════════════════════════════════════════════════════════════════════════

// In-memory storage
const db = {
    users: [
        { id: "user_1", name: "Alice Johnson", email: "alice@example.com", created_at: "2024-01-15" },
        { id: "user_2", name: "Bob Smith", email: "bob@example.com", created_at: "2024-02-20" },
        { id: "user_3", name: "Carol White", email: "carol@example.com", created_at: "2024-03-10" }
    ] as User[],

    products: [
        { id: "prod_1", name: "Laptop", price: 999.99, stock: 50 },
        { id: "prod_2", name: "Mouse", price: 29.99, stock: 200 },
        { id: "prod_3", name: "Keyboard", price: 79.99, stock: 5 },
        { id: "prod_4", name: "Monitor", price: 299.99, stock: 30 }
    ] as Product[],

    orders: [
        { id: "order_1", user_id: "user_1", product_id: "prod_1", quantity: 1, total: 999.99, status: "completed" as const },
        { id: "order_2", user_id: "user_1", product_id: "prod_2", quantity: 2, total: 59.98, status: "completed" as const },
        { id: "order_3", user_id: "user_2", product_id: "prod_3", quantity: 1, total: 79.99, status: "pending" as const },
        { id: "order_4", user_id: "user_3", product_id: "prod_1", quantity: 1, total: 999.99, status: "completed" as const }
    ] as Order[]
};

// ═══════════════════════════════════════════════════════════════════════════
// QUERY TOOLS
// ═══════════════════════════════════════════════════════════════════════════

export const db_query_users = async (args: { filter: string }) => {
    const { filter } = args;

    if (filter === "all") {
        return db.users;
    }

    if (filter.startsWith("id:")) {
        const id = filter.substring(3);
        const user = db.users.find(u => u.id === id);
        return [user]
    }

    if (filter.startsWith("email:")) {
        const email = filter.substring(6);
        const user = db.users.find(u => u.email === email);
        return [user]
    }

    return []
};

export const db_query_products = async (args: { filter: string }) => {
    const { filter } = args;

    if (filter === "all") {
        return db.products;
    }

    if (filter.startsWith("id:")) {
        const id = filter.substring(3);
        const product = db.products.find(p => p.id === id);
        return [product]
    }

    if (filter.startsWith("name:")) {
        const name = filter.substring(5);
        const product = db.products.find(p => p.name.toLowerCase().includes(name.toLowerCase()));
        return [product]
    }

    return []
};

export const db_query_orders = async (args: { filter: string }) => {
    const { filter } = args;

    if (filter === "all") {
        return db.orders;
    }

    if (filter.startsWith("user_id:")) {
        const user_id = filter.substring(8);
        const orders = db.orders.filter(o => o.user_id === user_id);
        return orders;
    }

    if (filter.startsWith("status:")) {
        const status = filter.substring(7) as "pending" | "completed" | "cancelled";
        const orders = db.orders.filter(o => o.status === status);
        return orders;
    }

    return []
};

// ═══════════════════════════════════════════════════════════════════════════
// CREATE TOOLS
// ═══════════════════════════════════════════════════════════════════════════

export const db_create_user = async (args: { name: string; email: string }) => {
    const { name, email } = args;

    const newUser: User = {
        id: `user_${db.users.length + 1}`,
        name,
        email,
        created_at: new Date().toISOString().split('T')[0]
    };

    db.users.push(newUser);
    return newUser
};

export const db_create_product = async (args: { name: string; price: number; stock: number }) => {
    const { name, price, stock } = args;

    const newProduct: Product = {
        id: `prod_${db.products.length + 1}`,
        name,
        price,
        stock
    };

    db.products.push(newProduct);
    return newProduct
};

export const db_create_order = async (args: { user_id: string; product_id: string; quantity: number }) => {
    const { user_id, product_id, quantity } = args;

    // Validate user exists
    const user = db.users.find(u => u.id === user_id);
    if (!user) {
        return JSON.stringify({ success: false, error: "User not found" });
    }

    // Validate product exists and has stock
    const product = db.products.find(p => p.id === product_id);
    if (!product) {
        return JSON.stringify({ success: false, error: "Product not found" });
    }

    if (product.stock < quantity) {
        return JSON.stringify({ success: false, error: "Insufficient stock" });
    }

    // Create order
    const newOrder: Order = {
        id: `order_${db.orders.length + 1}`,
        user_id,
        product_id,
        quantity,
        total: product.price * quantity,
        status: "pending"
    };

    // Update stock
    product.stock -= quantity;

    db.orders.push(newOrder);
    return JSON.stringify({ success: true, order: newOrder });
};

// ═══════════════════════════════════════════════════════════════════════════
// HELPER-SPECIFIC TOOLS (DataAnalyzer)
// ═══════════════════════════════════════════════════════════════════════════

export const analyze_user_behavior = async (args: { user_id: string }) => {
    const { user_id } = args;

    const userOrders = db.orders.filter(o => o.user_id === user_id);
    const totalSpent = userOrders.reduce((sum, order) => sum + order.total, 0);
    const completedOrders = userOrders.filter(o => o.status === "completed").length;

    return JSON.stringify({
        user_id,
        total_orders: userOrders.length,
        completed_orders: completedOrders,
        total_spent: totalSpent,
        average_order_value: userOrders.length > 0 ? totalSpent / userOrders.length : 0,
        insights: [
            totalSpent > 1000 ? "High-value customer" : "Regular customer",
            completedOrders === userOrders.length ? "Reliable completion rate" : "Has pending orders"
        ]
    }, null, 2);
};

export const detect_low_stock = async () => {
    const lowStockProducts = db.products.filter(p => p.stock < 10);

    if (lowStockProducts.length === 0) {
        return JSON.stringify({ message: "All products have sufficient stock" });
    }

    return JSON.stringify({
        alert: "Low stock detected",
        products: lowStockProducts.map(p => ({
            id: p.id,
            name: p.name,
            current_stock: p.stock,
            recommended_reorder: 50
        }))
    }, null, 2);
};

// Workflow-scoped tools for DataAnalyzer
export const calculate_average = async (args: { numbers: string }) => {
    const { numbers } = args;
    const nums = numbers.split(',').map(n => parseFloat(n.trim())).filter(n => !isNaN(n));
    if (nums.length === 0) return 0;
    return nums.reduce((a, b) => a + b, 0) / nums.length;
};

export const find_outliers = async (args: { data: string }) => {
    const { data } = args;
    // Simple outlier detection - values more than 2 std devs from mean
    const nums = data.split(',').map(n => parseFloat(n.trim())).filter(n => !isNaN(n));
    if (nums.length < 3) return "Not enough data for outlier detection";

    const mean = nums.reduce((a, b) => a + b, 0) / nums.length;
    const variance = nums.reduce((sum, n) => sum + Math.pow(n - mean, 2), 0) / nums.length;
    const stdDev = Math.sqrt(variance);

    const outliers = nums.filter(n => Math.abs(n - mean) > 2 * stdDev);
    return outliers.length > 0 ? `Outliers found: ${outliers.join(', ')}` : "No outliers detected";
};

// ═══════════════════════════════════════════════════════════════════════════
// HELPER-SPECIFIC TOOLS (ReportGenerator)
// ═══════════════════════════════════════════════════════════════════════════

export const format_table = async (args: { data: string }) => {
    const { data } = args;

    try {
        const parsed = JSON.parse(data);

        // Simple text table formatting
        if (Array.isArray(parsed)) {
            const headers = Object.keys(parsed[0] || {});
            const rows = parsed.map(item =>
                headers.map(h => String(item[h] || "")).join(" | ")
            );

            return headers.join(" | ") + "\n" + "-".repeat(50) + "\n" + rows.join("\n");
        }

        // Single object
        return Object.entries(parsed)
            .map(([key, value]) => `${key}: ${value}`)
            .join("\n");
    } catch (e) {
        return "Error formatting table: " + data;
    }
};

export const generate_chart_description = async (args: { data: string; chart_type: string }) => {
    const { data, chart_type } = args;

    try {
        const parsed = JSON.parse(data);

        if (chart_type === "bar") {
            return `Bar Chart: Showing ${Object.keys(parsed).length} data points. ` +
                   `Values range from ${Math.min(...Object.values(parsed).map(Number))} to ${Math.max(...Object.values(parsed).map(Number))}.`;
        }

        return `${chart_type} chart with data: ${data}`;
    } catch (e) {
        return `Chart description for ${chart_type}: ${data}`;
    }
};

// Workflow-scoped tools for ReportGenerator
export const aggregate_by_status = async (args: { orders: string }) => {
    const { orders } = args;
    try {
        const parsed = JSON.parse(orders);
        const statusCounts: Record<string, number> = {};

        if (Array.isArray(parsed)) {
            parsed.forEach((order: any) => {
                const status = order.status || "unknown";
                statusCounts[status] = (statusCounts[status] || 0) + 1;
            });
        }

        return JSON.stringify(statusCounts, null, 2);
    } catch (e) {
        return JSON.stringify({ error: "Failed to aggregate orders" });
    }
};

export const calculate_metrics = async (args: { orders: string }) => {
    const { orders } = args;
    try {
        const parsed = JSON.parse(orders);

        if (!Array.isArray(parsed)) {
            return JSON.stringify({ error: "Invalid orders data" });
        }

        const totalRevenue = parsed.reduce((sum: number, order: any) => sum + (order.total || 0), 0);
        const avgOrderValue = parsed.length > 0 ? totalRevenue / parsed.length : 0;

        return JSON.stringify({
            total_orders: parsed.length,
            total_revenue: totalRevenue.toFixed(2),
            average_order_value: avgOrderValue.toFixed(2)
        }, null, 2);
    } catch (e) {
        return JSON.stringify({ error: "Failed to calculate metrics" });
    }
};

// ═══════════════════════════════════════════════════════════════════════════
// SCOPED WORKFLOW TOOLS (These are called from within workflows)
// ═══════════════════════════════════════════════════════════════════════════

export const sum_order_totals = async (args: { orders_json: string }) => {
    try {
        const orders = JSON.parse(args.orders_json) as Order[];
        const total = orders.reduce((sum, order) => sum + order.total, 0);
        return total;
    } catch (e) {
        return 0;
    }
};

export const validate_stock = async (args: { product_id: string; quantity: number }) => {
    const product = db.products.find(p => p.id === args.product_id);
    return product ? product.stock >= args.quantity : false;
};

export const parse_csv = async (args: { csv_string: string }) => {
    return args.csv_string.split(',').map(s => s.trim()).join(', ');
};

// ═══════════════════════════════════════════════════════════════════════════
// EXPORT ALL TOOLS
// ═══════════════════════════════════════════════════════════════════════════

export const tools: MainTools = {
    // Main agent tools
    db_query_users,
    db_query_products,
    db_query_orders,
    db_create_user,
    db_create_product,
    db_create_order,

    // Workflow-scoped tools
    sum_order_totals,
    validate_stock,
    parse_csv,

    // DataAnalyzer helper tools
    analyze_user_behavior,
    detect_low_stock,
    calculate_average,
    find_outliers,

    // ReportGenerator helper tools
    format_table,
    generate_chart_description,
    aggregate_by_status,
    calculate_metrics
};
