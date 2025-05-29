import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import Logo from "@/components/ui/logo";
import {
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuSeparator,
	DropdownMenuLabel,
	DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import Link from "next/link";
import { createClient } from "@/utils/supabase/client";

export default function JobPageHeader(props: {
	fullName: string | null;
	avatarUrl: string | null;
}) {
	const avatarFallbackInitials = props.fullName
		?.split(" ")
		.map(name => name[0])
		.join("")

	return (
		<header className="bg-background flex h-14 items-center justify-between border-b px-6">
			<Logo></Logo>
			<h2 className="text-sm font-semibold">Your Job Posts</h2>
			<DropdownMenu>
				<DropdownMenuTrigger asChild>
					<Avatar className="h-9 w-9">
						<AvatarImage src={props.avatarUrl || ""} alt="User profile picture" />
						<AvatarFallback>{avatarFallbackInitials || ""}</AvatarFallback>
						<span className="sr-only">Toggle user menu</span>
					</Avatar>
				</DropdownMenuTrigger>
				<DropdownMenuContent className="w-56">
					<DropdownMenuLabel className="text-lg font-bold">
						{props.fullName}
					</DropdownMenuLabel>
					<DropdownMenuSeparator />
					<DropdownMenuItem asChild>
						<Link
							href="/settings"
							className="block w-full text-left"
						>
							Settings
						</Link>
					</DropdownMenuItem>
					<DropdownMenuSeparator />
					<DropdownMenuItem asChild>
						<form action="/api/auth/signout" method="post">
							<button className="block w-full text-left" type="submit">
								Sign out
							</button>
						</form>
					</DropdownMenuItem>
				</DropdownMenuContent>
			</DropdownMenu>
		</header>
	);
}
